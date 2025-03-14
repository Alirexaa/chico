use hyper::body::{Body, Bytes};

#[derive(Clone, Copy)]
pub struct MockBody {
    data: &'static [u8],
}

impl MockBody {
    pub fn new(data: &'static [u8]) -> Self {
        Self { data }
    }
}

impl Body for MockBody {
    type Data = Bytes;
    type Error = hyper::Error;

    fn poll_frame(
        mut self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Result<hyper::body::Frame<Self::Data>, Self::Error>>> {
        if self.data.is_empty() {
            std::task::Poll::Ready(None)
        } else {
            let data = self.data;
            self.data = &[];
            std::task::Poll::Ready(Some(Ok(hyper::body::Frame::data(Bytes::from(data)))))
        }
    }
}
