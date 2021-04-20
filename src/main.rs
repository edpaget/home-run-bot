use serde::Deserialize;
use serde_json::{json, Value};
use chrono::prelude::Utc;
use chrono_tz::US::Pacific;
use tokio::time::{sleep, Duration};

#[derive(Deserialize, Debug)]
struct PlaybackData {
    name: String,
    url: String,
}

#[derive(Deserialize, Debug)]
struct FeedData {
    r#type: String,
    playbacks: Vec<PlaybackData>
}

#[derive(Deserialize, Debug)]
struct MediaPlaybackData {
    id: String,
    description: String,
    feeds: Vec<FeedData>,
}

#[allow(non_snake_case)]
#[derive(Deserialize, Debug)]
struct MediaPlaybackResponse {
    mediaPlayback: [MediaPlaybackData; 1]
}

#[derive(Deserialize, Debug)]
struct PlayResponse {
    plays: Vec<MediaPlaybackResponse>
}
    
#[derive(Deserialize, Debug)]
struct SearchResponse {
    search: PlayResponse
}

#[derive(Deserialize, Debug)]
struct GraphQLResponse {
    data: SearchResponse
}

const FIVE_MINUTES: u64 = 300000;

const BASE_URL: &str = "https://fastball-gateway.mlb.com/graphql";

const SEARCH_QUERY: &str = "query Search($query: String!, $page: Int, $limit: Int, $feedPreference: FeedPreference, $languagePreference: LanguagePreference, $contentPreference: ContentPreference) { search(query: $query, limit: $limit, page: $page, feedPreference: $feedPreference, languagePreference: $languagePreference, contentPreference: $contentPreference) { plays { mediaPlayback { ...MediaPlaybackFields __typename } __typename } total __typename } } fragment MediaPlaybackFields on MediaPlayback { id description feeds { type playbacks { name url __typename } __typename } __typename }";

const SLACK_WEBHOOK: &str = "https://hooks.slack.com/services/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";

async fn query_mlb(client: &reqwest::Client) -> Result<GraphQLResponse, Box<dyn std::error::Error>> {
    let query = format!(
        "HitResult = [\"Home Run\"] AND Date = [\"{}\"] Order By Timestamp DESC",
        Utc::today().with_timezone(&Pacific).format("%Y-%m-%d")
    );
    let search_vars = json!({
            "query": query,
            "limit": 1,
            "page": 0,
            "languagePrefrence": "EN",
            "contentPreference": "MIXED"
            
    });
    Ok(client.get(BASE_URL)
       .header(reqwest::header::USER_AGENT, "HomeRunBot/1.0")
       .header(reqwest::header::CONTENT_TYPE, "application/json")
       .query(&[
           ("query", SEARCH_QUERY),
           ("operationName", "Search"),
           ("variables", &search_vars.to_string())
       ]).send()
       .await?
       .json::<GraphQLResponse>()
       .await?)
}

fn build_slack_request(playback: &MediaPlaybackData) -> Result<Value, Box<dyn std::error::Error>> {
    let cms_feed = playback.feeds.iter()
        .find(|  feed| feed.r#type == "CMS").unwrap();
    let hq_playback =  cms_feed.playbacks.iter()
        .find(|  playback| playback.name == "mp4Avc").unwrap();
    Ok(json!({
        "text": format!("{} {}", playback.description, hq_playback.url)
    }))
}

async fn send_slack(client: &reqwest::Client, slack_post: &Value) -> Result<String, Box<dyn std::error::Error>> {
    Ok(client.post(SLACK_WEBHOOK)
       .header(reqwest::header::USER_AGENT, "HomeRunBot/1.0")
       .json(slack_post)
       .send()
       .await?
       .text()
       .await?)
}


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let mut last_id: String = "".to_string();
    loop {
        let mlb_response = query_mlb(&client).await?;
        println!("{:?}", mlb_response);
        if let [play] = &mlb_response.data.search.plays[..] {
            let playback = &play.mediaPlayback[0];
            println!("{}", playback.id);
            if playback.id != last_id {
                last_id = playback.id.clone();
                send_slack(&client, &build_slack_request(playback).unwrap()).await?;
            }
        }
        sleep(Duration::from_millis(FIVE_MINUTES)).await;
    }
}
