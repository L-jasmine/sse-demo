use futures_util::StreamExt;
use futures_util::TryStreamExt;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server, StatusCode};
use std::pin::Pin;
use std::{net::SocketAddr, time::Duration};
use tokio::time::interval;

type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

async fn hello_stream() -> Result<impl futures::TryStream<Ok = String, Error = String>, String> {
    let mut interval = interval(Duration::from_secs(1));

    let event_stream = async_stream::try_stream! {
        // loop {
        //     interval.tick().await;
        //     yield "data: This is a message\n\n".to_string();
        // }

        let mut cnt = 0;
        while cnt < 3 {
            cnt += 1;
            interval.tick().await;
            yield format!("data: This is message_{}\n\n", cnt);
        }
    };

    Ok(event_stream)
}

struct EventStream {
    count: u32,
    stream: Pin<Box<dyn futures::Stream<Item = Result<String, String>> + Send>>,
}

impl futures::Stream for EventStream {
    type Item = Result<String, String>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let me = self.get_mut();
        let pin_s = me.stream.as_mut();
        match futures::Stream::poll_next(pin_s, cx) {
            std::task::Poll::Ready(Some(Ok(data))) => {
                if me.count > 0 {
                    me.count -= 1;
                }
                if me.count == 0 {
                    println!("data is {:?}", data);
                    std::task::Poll::Ready(Some(Ok(data)))
                } else {
                    std::task::Poll::Ready(Some(Ok(data)))
                }
            }
            std::task::Poll::Ready(Some(Err(e))) => std::task::Poll::Ready(Some(Err(e))),
            std::task::Poll::Ready(None) => std::task::Poll::Ready(None),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}

async fn handle_request(_req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
    let stream = hello_stream().await.unwrap();

    let event_stream = stream.map_err(|e| e.to_string());
    let event_stream = EventStream {
        count: 3,
        stream: Box::pin(event_stream),
    };

    // ! debug
    // ==================================================>

    // // Collect the stream into a vector
    // let mut data = stream.try_collect::<Vec<String>>().await.unwrap();

    // // Process the data here...
    // println!("{:?}", &data);

    // // Convert the vector back into a stream
    // let event_stream = futures::stream::iter(data).map(Ok::<_, String>);

    // <==================================================

    let body = Body::wrap_stream(event_stream);

    let response = Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/event-stream")
        .body(body)
        .unwrap();

    Ok(response)
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let make_svc = make_service_fn(|_conn| async move {
        Ok::<_, Error>(service_fn(move |req| handle_request(req)))
    });

    let addr = SocketAddr::from(([127, 0, 0, 1], 9069));

    let server = Server::bind(&addr).serve(make_svc);

    println!("[INFO] Listening on http://{}", addr);

    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }
}
