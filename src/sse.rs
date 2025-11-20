use std::{
    convert::Infallible,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures_core::Stream;
use http_body::Frame;
use tokio::sync::broadcast::Receiver;
use tokio_stream::wrappers::BroadcastStream;

pub struct ReloadEventsBody {
    stream: Option<BroadcastStream<()>>,
    sent_init: bool,
    retry_duration: Duration,
}

impl ReloadEventsBody {
    pub fn new(receiver: Receiver<()>, retry_duration: Duration) -> Self {
        Self {
            stream: Some(BroadcastStream::new(receiver)),
            sent_init: false,
            retry_duration,
        }
    }
}

impl http_body::Body for ReloadEventsBody {
    type Data = bytes::Bytes;
    type Error = Infallible;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        // First, send the init event if we haven't yet
        if !self.sent_init {
            self.sent_init = true;
            return Poll::Ready(Some(Ok(Frame::data(bytes::Bytes::from_owner(format!(
                "event: init\ndata:\nretry: {}\n\n",
                self.retry_duration.as_millis()
            ))))));
        }

        // Then wait for reload messages
        if let Some(stream) = &mut self.stream {
            let stream_pin = Pin::new(stream);
            match stream_pin.poll_next(cx) {
                Poll::Ready(Some(Ok(_))) | Poll::Ready(Some(Err(_))) => {
                    // Got a reload message or lagged
                    self.stream = None;
                    Poll::Ready(Some(Ok(Frame::data(bytes::Bytes::from_static(
                        b"event: reload\ndata:\n\n",
                    )))))
                }
                Poll::Ready(None) => {
                    // Stream closed
                    self.stream = None;
                    Poll::Ready(None)
                }
                Poll::Pending => {
                    // Waiting for message
                    Poll::Pending
                }
            }
        } else {
            // Already sent reload or closed
            Poll::Ready(None)
        }
    }
}
