mod game_server;

use std::fs;

use axum::{
    extract::Path,
    response::{Html, Response},
    routing::get,
    Router, http::StatusCode,
    body::Body,
};

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", get(index))
        .route("/files/{object}/{file_name}", get(return_file));

    game_server::game_server::start_server();

    let listener = tokio::net::TcpListener::bind("0.0.0.0:9090").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn index() -> Html<String> {
    let content_home = content("./files/html/index.html");

    Html(content_home)
}

async fn return_file(Path((object, file_name)): Path<(String, String)>) -> Response {
    let file_path = format!("./files/{object}/{file_name}");

    let file = fs::read(&file_path[..]);

    match file {
        Ok(v) => 
        Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "text/".to_owned() + &object)
            .body(Body::from(v))
            .unwrap(),
        Err(e) => {
            println!("File not found {}: {:?}", file_path, e);

            Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from(""))
                .unwrap()
        },
    }
}

fn content(file_path: &str) -> String {
    let content = fs::read_to_string(file_path).expect("Valid file");

    content
}