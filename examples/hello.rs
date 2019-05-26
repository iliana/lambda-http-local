use http::Response;

fn main() {
    lambda_http_local::run(
        |_request| Response::new(b"Hello, world!".to_vec()),
        "localhost:3000",
    );
}
