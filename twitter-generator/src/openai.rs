use http_body_util::BodyExt;
use hyper::{
    body::Buf,
    header::{AUTHORIZATION, CONTENT_TYPE, HOST},
    Request, StatusCode, Uri,
};
use hyper_util::rt::TokioIo;
use tokio::net::TcpStream;
// use reqwest::{
//     header::{AUTHORIZATION, CONTENT_TYPE},
//     Error,
// };
use serde::{Deserialize, Serialize};
use std::env;

pub struct OpenAI {}

impl OpenAI {
    fn fetch_api_key() -> String {
        env::var("OPENAI_API_KEY").expect("$OPENAI_API_KEY is not set.")
    }

    pub async fn request(
        &self,
        request: OpenAIRequest,
    ) -> Result<OpenAIResponse, Box<dyn std::error::Error>> {
        let api_token = format!("Bearer {}", OpenAI::fetch_api_key());
        // let req_type = match request.endpoint_enum {
        //     OpenAIEndpoint::Chat => "chat",
        //     OpenAIEndpoint::Images => "image",
        // };
        let endpoint = request.endpoint.clone().parse::<Uri>()?;
        let endpoint_address = format!(
            "{}:{}",
            endpoint
                .clone()
                .host()
                .expect("Can't get a host from the API url."),
            443
        );

        let stream = TcpStream::connect(endpoint_address).await?;
        let io = TokioIo::new(stream);

        let (mut sender, conn) =
            hyper::client::conn::http1::handshake::<TokioIo<TcpStream>, String>(io).await?;

        tokio::task::spawn(async move {
            conn.await.expect("Unable to connect to OpenAI servers.");
        });

        let req_body = match request.endpoint_enum {
            OpenAIEndpoint::Chat => serde_json::to_string(&request.chat_req)?,
            OpenAIEndpoint::Images => serde_json::to_string(&request.image_req)?,
        };

        let req = Request::builder()
            .method("POST")
            .uri(endpoint.clone())
            .header(
                HOST,
                endpoint.authority().expect("Invalid API URI.").as_str(),
            )
            .header(CONTENT_TYPE, "application/json")
            .header(AUTHORIZATION, api_token.clone())
            .body(req_body)?;

        let resp = sender.send_request(req).await?;
        let status = resp.status();
        if status != StatusCode::OK {
            dbg!(resp.collect().await?);
            return Err("Invalid OpenAI request response.".into());
        }
        let response_json: String =
            serde_json::from_reader(resp.collect().await?.aggregate().reader())?;
        match request.endpoint_enum {
            OpenAIEndpoint::Chat => Ok(OpenAIResponse {
                chat: Some(serde_json::from_str::<ChatCompletionsResponse>(
                    &response_json,
                )?),
                image: None,
            }),
            OpenAIEndpoint::Images => Ok(OpenAIResponse {
                image: Some(serde_json::from_str::<ImageGenerationResponse>(
                    &response_json,
                )?),
                chat: None,
            }),
        }
        // if req_type == "chat" {
        //     // http_request = http_request.json(&request.chat_req.unwrap().clone());
        //     if let Some(req) = &request.chat_req {
        //         println!("{}", serde_json::to_string(req).unwrap().to_string());
        //         let response = client
        //             .post(request.endpoint.clone())
        //             .header(CONTENT_TYPE, "application/json")
        //             .header(AUTHORIZATION, api_token.clone())
        //             .json(&req);
        //         dbg!(&response);
        //
        //         // response = response.send().unwrap().json::<ChatCompletionsResponse>()?;
        //         return Ok(OpenAIResponse {
        //             chat: Some(response.send().unwrap().json::<ChatCompletionsResponse>()?),
        //             image: None,
        //         });
        //     };
        //     // dbg!(&http_request);
        //     let response = http_request
        //         .send()
        //         .unwrap()
        //         .json::<ChatCompletionsResponse>()?;
        //     return Ok(OpenAIResponse {
        //         chat: Some(response),
        //         image: None,
        //     });
        // } else {
        //     let mut http_request = client
        //         .post(request.endpoint.clone())
        //         .header(CONTENT_TYPE, "application/json")
        //         .header(AUTHORIZATION, api_token.clone());
        //     http_request = http_request.json(&request.image_req.unwrap());
        //     let response = http_request
        //         .send()
        //         .unwrap()
        //         .json::<ImageGenerationResponse>()?;
        //     return Ok(OpenAIResponse {
        //         chat: None,
        //         image: Some(response),
        //     });
        // }
    }
}

#[allow(non_snake_case)]
#[derive(Debug)]
pub enum OpenAIEndpoint {
    Chat,
    Images,
}

impl OpenAIEndpoint {
    pub fn url(&self) -> &str {
        match self {
            Self::Chat => "https://api.openai.com/v1/chat/completions",
            Self::Images => "https://api.openai.com/v1/images/generations",
        }
    }
}

pub enum OpenAIModels {
    GPT35Turbo,
    GPT35TurboInstruct,
    Dalle2,
}

impl OpenAIModels {
    pub fn name(&self) -> &str {
        match self {
            Self::GPT35Turbo => "gpt-3.5-turbo",
            Self::GPT35TurboInstruct => "gpt-3.5-turbo-instruct",
            Self::Dalle2 => "dall-e-2",
        }
    }
}

#[derive(Debug)]
pub struct OpenAIRequest {
    pub endpoint: String,
    pub endpoint_enum: OpenAIEndpoint,
    pub chat_req: Option<ChatRequest>,
    pub image_req: Option<ImageRequest>,
}

#[derive(Debug, Serialize)]
pub struct ChatRequest {
    model: String,
    #[serde()]
    messages: Vec<Message>,
    temperature: f64,
    max_tokens: u32,
}

#[derive(Debug, Serialize)]
pub struct ImageRequest {
    model: String,
    messages: Option<Vec<Message>>,
    temperature: Option<f64>,
    max_tokens: Option<u32>,
}

const SYSTEM_MESSAGE: &str = "You are a content creator who is speacialized in making content which will generate a lot of engagement. You focus on making sure to take a side on the issue so that people can reply to your tweet with clear agreement or dissent.";

impl OpenAIRequest {
    pub fn new(endpoint: OpenAIEndpoint) -> Self {
        Self {
            endpoint: endpoint.url().to_string(),
            endpoint_enum: endpoint,
            chat_req: None,
            image_req: None,
        }
    }

    pub fn chat_req(
        mut self,
        model: OpenAIModels,
        message: &str,
        temperature: f64,
        max_tokens: u32,
    ) -> Self {
        self.chat_req = Some(ChatRequest {
            model: model.name().to_string(),
            messages: vec![
                Message {
                    role: "system".to_string(),
                    content: SYSTEM_MESSAGE.to_string(),
                },
                Message {
                    role: "user".to_string(),
                    content: message.to_string(),
                },
            ],
            temperature,
            max_tokens,
        });
        self
    }
}

#[derive(Debug)]
pub struct OpenAIResponse {
    pub chat: Option<ChatCompletionsResponse>,
    pub image: Option<ImageGenerationResponse>,
}

#[derive(Debug, Deserialize)]
pub struct ImageGenerationResponse {
    created: u32,
    data: Vec<Url>,
}

#[derive(Debug, Deserialize)]
pub struct Url {
    url: String,
}

#[derive(Debug, Deserialize)]
pub struct ChatCompletionsResponse {
    id: Option<String>,
    object: Option<String>,
    created: Option<u32>,
    model: Option<String>,
    system_fingerprint: Option<String>,
    choices: Option<Vec<Choice>>,
    usage: Option<Usage>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
struct Choice {
    index: u32,
    message: Message,
    log_probs: Option<String>,
    finish_reason: String,
}

#[derive(Debug, Deserialize)]
struct Usage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}
