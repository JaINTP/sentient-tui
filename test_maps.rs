#[tokio::main]
async fn main() {
    let client = reqwest::Client::new();
    let url = "http://127.0.0.1:8002/api/v1/maps?page=1&size=10";
    let resp = client.get(url).send().await.unwrap();
    println!("Status: {}", resp.status());
    let text = resp.text().await.unwrap();
    println!("Body: {}", text);
}
