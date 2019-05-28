//! A small wrapper for locally developing HTTP handlers for AWS Lambda.
//!
//! For details, see the documentation for [`run`](fn.run.html).

#![warn(clippy::pedantic)]

use http::{Request, Response};
use std::net::ToSocketAddrs;

/// Run an HTTP handler in either AWS Lambda or as a local development HTTP server.
///
/// If the `AWS_LAMBDA_RUNTIME_API` environment variable is present (as it is when running in
/// Lambda), requests are received via the [AWS Lambda Runtime Interface][lambda-interface].
///
/// If not, an HTTP server is started on `listen_addr`.
///
/// If you need to decrease the size of the binary you deploy to Lambda, you can build the crate
/// with `--no-default-features` for your production build, which disables the HTTP server
/// component.
///
/// [lambda-interface]: https://docs.aws.amazon.com/lambda/latest/dg/runtimes-api.html
///
/// # Example
///
/// ```rust,no_run
/// # use http::Response;
/// lambda_http_local::run(
///     |request| Response::new(b"Hello, world!".to_vec()),
///     "localhost:3000",
/// );
/// ```
///
/// # Panics
///
/// If `AWS_LAMBDA_RUNTIME_API` is not present and `listen_addr`'s [`to_socket_addrs`] method fails
/// or resolves to no [`SocketAddr`] values, this function will panic.
///
/// [`to_socket_addrs`]: https://doc.rust-lang.org/std/net/addr/trait.ToSocketAddrs.html#tymethod.to_socket_addrs
/// [`SocketAddr`]: https://doc.rust-lang.org/std/net/addr/enum.SocketAddr.html
///
/// # Lambda context object
///
/// When running in AWS Lambda, the [`Context`] object for the request is available as a [request
/// extension].
///
/// [`Context`]: ../lambda_runtime_core/context/struct.Context.html
/// [request extension]: ../http/request/struct.Request.html#method.extensions
#[cfg_attr(not(feature = "local"), allow(unused_variables))]
pub fn run<F, T>(handler: F, listen_addr: T)
where
    F: Fn(Request<&[u8]>) -> Response<Vec<u8>> + Send + Sync + 'static,
    T: ToSocketAddrs,
{
    #[cfg(feature = "local")]
    {
        if is_lambda() {
            // AWS Lambda mode
            lambda(handler)
        } else {
            use hyper::rt::{Future, Stream};
            use std::sync::Arc;

            // Hyper server mode
            let listen_addr = listen_addr
                .to_socket_addrs()
                .expect("listen_addr.to_socket_addrs() failed")
                .next()
                .expect("listen_addr.to_socket_addrs() resolved to no addresses");
            let handler = Arc::new(handler);
            let make_service = move || {
                let handler = handler.clone();
                hyper::service::service_fn(move |request: Request<hyper::Body>| {
                    let handler = handler.clone();
                    let (parts, body) = request.into_parts();
                    body.concat2().map(move |chunk| {
                        handler(Request::from_parts(parts, &chunk)).map(hyper::Body::from)
                    })
                })
            };
            let server = hyper::Server::bind(&listen_addr).serve(make_service);
            eprintln!("Listening on http://{}", listen_addr);
            hyper::rt::run(server.map_err(|e| {
                eprintln!("Hyper error: {}", e);
            }));
        }
    }

    #[cfg(not(feature = "local"))]
    {
        lambda(handler)
    }
}

pub fn is_lambda() -> bool {
    #[cfg(feature = "local")]
    {
        return std::env::var_os("AWS_LAMBDA_RUNTIME_API").is_some();
    }

    #[cfg(not(feature = "local"))]
    {
        return true;
    }
}

fn lambda<F>(handler: F)
where
    F: Fn(Request<&[u8]>) -> Response<Vec<u8>> + Send + Sync + 'static,
{
    lambda_http::lambda!(|request: Request<lambda_http::Body>, context| {
        let (mut parts, body) = request.into_parts();
        parts.extensions.insert(context);
        Ok(handler(Request::from_parts(parts, body.as_ref())).map(lambda_http::Body::from))
    })
}
