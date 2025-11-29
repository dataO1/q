use crate::api_client::ApiClient;


pub async fn execute(query: &str) -> anyhow::Result<()> {
    let client = ApiClient::new("http://localhost:8080").await?;
    let response = client.query(query).await?;
    println!("Response:\n{}", response);
    Ok(())
}
