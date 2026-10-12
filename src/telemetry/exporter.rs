use opentelemetry_proto::tonic::collector::metrics::v1::{
    ExportMetricsServiceRequest, metrics_service_client::MetricsServiceClient,
};
use opentelemetry_sdk::error::OTelSdkResult;
use opentelemetry_sdk::metrics::{
    Temporality, data::ResourceMetrics, exporter::PushMetricExporter,
};
use prost::Message;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;
use tokio::sync::Mutex;
use tonic::{Request, metadata::MetadataMap, transport::Channel};
use tracing::{debug, info, warn};

/// OTLP/gRPC exporter that keeps the requests the backend did not accept and
/// sends them again, oldest first, once it is reachable. Every request keeps the
/// timestamps of the moment it was collected, so an outage leaves no gap.
pub struct QueueExporter {
    client: MetricsServiceClient<Channel>,
    metadata: MetadataMap,
    /// Encoded requests: a third of the memory of the decoded structs.
    queue: Mutex<VecDeque<Vec<u8>>>,
    max: usize,
    queued: Arc<AtomicU64>,
    offline: Arc<AtomicBool>,
}

impl QueueExporter {
    pub fn new(
        channel: Channel,
        metadata: MetadataMap,
        max: usize,
        queued: Arc<AtomicU64>,
        offline: Arc<AtomicBool>,
    ) -> Self {
        Self {
            client: MetricsServiceClient::new(channel),
            metadata,
            queue: Mutex::new(VecDeque::new()),
            max: max.max(1),
            queued,
            offline,
        }
    }
}

impl PushMetricExporter for QueueExporter {
    async fn export(&self, metrics: &ResourceMetrics) -> OTelSdkResult {
        let mut queue = self.queue.lock().await;
        if queue.len() >= self.max {
            queue.pop_front();
        }
        let encoded = ExportMetricsServiceRequest::from(metrics).encode_to_vec();
        queue.push_back(encoded);

        // ponytail: drains the whole backlog in one go; the next collection waits
        // until it is sent (a few seconds for 72 h of data).
        let backlog = queue.len();
        let mut client = self.client.clone();
        while let Some(bytes) = queue.front() {
            let Ok(req) = ExportMetricsServiceRequest::decode(bytes.as_slice()) else {
                queue.pop_front();
                continue;
            };
            let mut request = Request::new(req);
            *request.metadata_mut() = self.metadata.clone();
            if let Err(status) = client.export(request).await {
                self.queued.store(queue.len() as u64, Ordering::Relaxed);
                debug!(code = ?status.code(), error = %status.message(), queued = queue.len(), "export failed");
                if !self.offline.swap(true, Ordering::Relaxed) {
                    warn!(error = %status.message(), "backend unreachable, keeping metrics until it is back");
                }
                // Queued, so nothing is lost: not an error for the SDK.
                return Ok(());
            }
            queue.pop_front();
        }
        self.queued.store(0, Ordering::Relaxed);
        if self.offline.swap(false, Ordering::Relaxed) {
            info!(
                sent = backlog,
                "backend reachable again, buffered metrics sent"
            );
        }
        Ok(())
    }

    fn force_flush(&self) -> OTelSdkResult {
        Ok(())
    }

    fn shutdown_with_timeout(&self, _timeout: Duration) -> OTelSdkResult {
        Ok(())
    }

    fn temporality(&self) -> Temporality {
        Temporality::Cumulative
    }
}
