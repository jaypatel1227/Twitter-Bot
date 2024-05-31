mod openai;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env::set_var("RUST_BACKTRACE", "full");
    let api = openai::OpenAI {};
    let mut req = openai::OpenAIRequest::new(openai::OpenAIEndpoint::Chat);
    req = req.chat_req(
        openai::OpenAIModels::GPT35TurboInstruct,
        "Write a tweet which would make a commentary on the global conflicts in the middle east.",
        1.0,
        200,
    );
    let resp = api.request(req).await?;
    dbg!(resp.chat);
    Ok(())
}
