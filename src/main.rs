use std::error::Error;
use axum::{routing::get, Router};
use dotenv::dotenv;
use reqwest;
use rss::Channel;
use serde::{Deserialize, Serialize};
use tokio::time::{self, Duration};
use rusqlite::{params, Connection, Result};
use chrono::{DateTime, FixedOffset, Utc, NaiveDateTime};
use rusqlite::Error as RusqliteError;
use regex::Regex;
use reqwest::header;
use serde_json::json;
use reqwest::{Client, RequestBuilder, Response, Error as ReqwestError};
use serde_json::Value;
use chrono_tz::Asia::Ho_Chi_Minh; 
use html_escape::decode_html_entities;
use reqwest::redirect::Policy;

#[derive(Debug, Deserialize, Serialize)]
struct RssItem {
    title: String,
    link: String,
    pubDate: String,
    #[serde(rename = "content:encoded")]
    content_encoded: String,
    #[serde(rename = "content:encodedSnippet")]
    content_encoded_snippet: String,
    content: String,
    contentSnippet: String,
    guid: String,
    isoDate: String,
}
impl Clone for RssItem {
  fn clone(&self) -> Self {
      // Clone each field from the inner RssItem
      RssItem {
        title: self.title.clone(),
        link: self.link.clone(),
        content_encoded: self.content_encoded.clone(),
        content_encoded_snippet: self.content_encoded_snippet.clone(),
        content: self.content.clone(),
        contentSnippet: self.contentSnippet.clone(),
        guid: self.guid.clone(),
        isoDate: self.isoDate.clone(),
        pubDate: self.pubDate.clone(),
      }
  }
}


#[tokio::main]
async fn main() {
    dotenv().ok();
    let db = get_db().await.expect("Failed to initialize database");
    let client = reqwest::Client::builder()
      .danger_accept_invalid_certs(true)
      .redirect(Policy::none())
      .use_rustls_tls()
      .build()
      .expect("Failed to build client");


    // Set up a repeating task
    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(15));
    
        loop {
          interval.tick().await;
          println!("Checking for new items...");
          let rss_items = read_rss_feed().await.expect("Failed to read RSS feed");
          
          let mut stmt = db.prepare("SELECT * FROM feed ORDER BY pub_date DESC LIMIT 1").expect("Failed to prepare SQL query");
          let row_result = match stmt.query_row(params![], |row| {
            let guid_result = row.get::<_, String>(3);
            let pub_date_result = row.get::<_, String>(2);

            match (guid_result, pub_date_result) {
              (Ok(guid_value), Ok(pub_date_str)) => {
                  // Parse the pub_date_str into DateTime<Utc>
                  match DateTime::parse_from_rfc3339(&pub_date_str) {
                      Ok(pub_date) => Ok((guid_value, pub_date.with_timezone(&Utc))),
                      Err(e) => Err(RusqliteError::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))),
                  }
              },
              _ => Err(RusqliteError::QueryReturnedNoRows), // or handle other errors as needed
            }
              }){
                Ok(row) => Some(row),
                Err(e) => {
                    if e == RusqliteError::QueryReturnedNoRows {
                        // Handle the case when no rows are returned
                        // You can put your logic here
                        None
                    } else {
                        // Handle other errors as needed
                        // You can print an error message or return None
                        println!("Error: {:?}", e);
                        None
                    }
                }
            };

            if let Some(ref row) = row_result {
              let slack_channel = std::env::var("SLACK_CHANNEL").expect("SLACK_CHANNEL must be set");
              for rss_item in rss_items {
                let pub_date = DateTime::parse_from_rfc2822(&rss_item.pubDate).expect("Failed to parse pub_date");
                if pub_date > row.1 {
                  // println!( "rss_items: {:?}", rss_items.content);
                  let rss_item_json = serde_json::to_string(&rss_item).expect("Failed to serialize rss_item");
                  // println!( "rss_item_json: {:?}", rss_item_json);
                  // let content_snippet = "We're looking for a visionary Front-End Developer... [rest of the string]";

                  let success = match db.execute(
                      "INSERT INTO feed (guid, pub_date, data) VALUES (?1, ?2, ?3)",
                      params![&rss_item.guid, &pub_date.to_rfc3339(), &rss_item_json],
                  ) {
                      Ok(_) =>{
                        println!("Inserted {}", &rss_item.guid);
                        true
                      },
                      Err(e) => {
                        println!("Failed to insert {}: {}", &rss_item.guid, e);
                        false
                      }
                  };
                  if !success {
                    continue;
                  }
                  // Create a Regex object
                  // let re = Regex::new(r"Posted On</b>: ([\w\s,:]+) UTC").unwrap();
              
                  // Perform the match
                  let summary = Regex::new(r"(?s)(.*?)<b>Hourly Range</b>:").unwrap().captures(&rss_item.contentSnippet)
                      .and_then(|caps| caps.get(1))
                      .map_or_else(|| "", |m| m.as_str());
                  let summary = Regex::new(r"<br />").unwrap().replace_all(&summary, "\n");
                  let hourly_range = Regex::new(r"<b>Hourly Range</b>:\s*([^\n<]+)").unwrap().captures(&rss_item.contentSnippet)
                    .and_then(|caps| caps.get(1))
                    .map_or_else(|| "", |m| m.as_str());
                  let location = Regex::new(r"<b>Country</b>:\s*([^\n<]+)").unwrap().captures(&rss_item.contentSnippet)
                    .and_then(|caps| caps.get(1))
                    .map_or_else(|| "", |m| m.as_str());
                  let category = Regex::new(r"<b>Category</b>:\s*([^\n<]+)").unwrap().captures(&rss_item.contentSnippet)
                    .and_then(|caps| caps.get(1))
                    .map_or_else(|| "", |m| m.as_str());
                  // println!( "location: {:?}", category);
                  // join skills
                  let skills_string: String = Regex::new(r"<b>Skills</b>:\s*([^<]+)")
                      .unwrap()
                      .captures(&rss_item.contentSnippet)
                      .and_then(|caps| caps.get(1).map(|skills_match| skills_match.as_str()))
                      .map_or_else(|| "".to_string(), |skills_str| {
                          let skills: Vec<&str> = skills_str.split(',').map(|s| s.trim()).collect();
                          skills.join(", ").to_string()
                      });

                  // Convert the skills_string to &str only when necessary
                  let skills: &str = &skills_string;

                  let utc_date = DateTime::parse_from_str(&format!("{}", &rss_item.pubDate), "%a, %d %b %Y %H:%M:%S %z").expect("Failed to parse posted_on").with_timezone(&Utc);
            
                  // Convert to Eastern Time
                  let est_date = utc_date.with_timezone(&Ho_Chi_Minh).format("%a, %d %b %Y %H:%M:%S").to_string();
                  
                  let message_body = json!({
                      "channel": &slack_channel,
                      "blocks": [
                        {
                          "type": "divider"
                        },
                        {
                          "type": "section",
                          "text": {
                            "type": "mrkdwn",
                            "text": format!("*{}*", &rss_item.title),
                          },
                        },
                        {
                          "type": "section",
                          "text": {
                            "type": "mrkdwn",
                            "text": decode_html_entities(&summary),
                          },
                        },
                        {
                          "type": "section",
                          "fields": [
                            {
                              "type": "mrkdwn",
                              "text": format!("*Posted On*: \n{}", &est_date),
                            },
                            {
                              "type": "mrkdwn",
                              "text": format!("*Hourly Range*: \n{}", &hourly_range),
                            },
                            {
                              "type": "mrkdwn",
                              "text": format!("*Location*: \n{}", &location),
                            },
                            {
                              "type": "mrkdwn",
                              "text": format!("*Category*: \n{}", &category),
                            },
                          ],
                        },
                        {
                          "type": "divider",
                        },
                        {
                          "type": "section",
                          "text": {
                            "type": "mrkdwn",
                            "text": "*Key Skills Required:*\n",
                          },
                        },
                        {
                          "type": "section",
                          "text": {
                            "type": "mrkdwn",
                            "text": &skills
                          },
                        },
                        {
                          "type": "actions",
                          "elements": [
                            {
                              "type": "button",
                              "text": {
                                "type": "plain_text",
                                "text": "Apply Now",
                              },
                              "url": &rss_item.guid,
                              "style": "primary",
                            },
                          ],
                        },
                      ]
                  });
                  println!("sending to slack: {:?}", &rss_item.title);
                  post_message(&client, &message_body);

                }
              }
            } else {
              for rss_item in rss_items {
                let rss_item_json = serde_json::to_string(&rss_item).expect("Failed to serialize rss_item");
                let pub_date = DateTime::parse_from_rfc2822(&rss_item.pubDate).expect("Failed to parse pub_date");
                match db.execute(
                    "INSERT INTO feed (guid, pub_date, data) VALUES (?1, ?2, ?3)",
                    params![&rss_item.guid, &pub_date.to_rfc3339(), &rss_item_json],
                ) {
                    Ok(_) => println!("Inserted {}", &rss_item.guid),
                    Err(e) => println!("Failed to insert {}: {}", &rss_item.guid, e),
                }
              }
            }
        }
    });

    // Read the PORT environment variable or use a default value (8080)
    let port = dotenv::var("PORT").unwrap_or_else(|_| "8080".to_string());
    // Combine the IP address and port
    let addr = format!("0.0.0.0:{}", port);
    // Set up Axum router and routes
    let app = Router::new().route("/", get(|| async { "Hello from Axum!" }));

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn get_db() -> Result<Connection> {
  dotenv::dotenv().ok();
  let db_path = dotenv::var("SQLITE_DB_PATH").expect("SQLITE_DB_PATH must be set");

  // Connect to the database or panic if it fails
  let db = Connection::open(db_path)
      .expect("Failed to connect to the database");

  setup_db(&db).await?;

  Ok(db)
}

async fn setup_db(db: &Connection) -> Result<()> {
  // Check if the 'feed' table exists
  let mut stmt = db.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='feed';").expect("Failed to prepare SQL query");
  let exists = stmt.exists(params![])?;

  // Create the table if it does not exist
  if !exists {
      db.execute(
          "CREATE TABLE IF NOT EXISTS feed (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              data TEXT,
              pub_date TIMESTAMP,
              guid NOT NULL UNIQUE,
              created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
          )",
          params![],
      )?;
  }

  Ok(())
}

async fn read_rss_feed() -> Result<Vec<RssItem>, Box<dyn Error>> {
  let rss_url = dotenv::var("RSS_URL").expect("RSS_URL must be set");

  let text = reqwest::get(rss_url).await?.text().await?;
  let channel = Channel::read_from(text.as_bytes())?;

  let items = channel.items().iter().map(|item| RssItem {
    title: item.title().unwrap_or_default().to_string(),
    link: item.link().unwrap_or_default().to_string(),
    pubDate: item.pub_date().unwrap_or_default().to_string(),
    content_encoded: item.content().unwrap_or_default().to_string(),
    content_encoded_snippet: item.content().unwrap_or_default().to_string(),
    content: item.content().unwrap_or_default().to_string(),
    contentSnippet: item.content().unwrap_or_default().to_string(),
    guid: match item.guid() {
        Some(g) => g.value().to_string(),
        None => String::new(), // or any default String value you prefer
    },
    isoDate: item.pub_date().unwrap_or_default().to_string(),   
  }).collect::<Vec<RssItem>>();

  Ok(items)
}

fn post_message(client: &Client, message_body: &Value) {
  let is_debug = dotenv::var("DEBUG").unwrap_or_default() == "true";
  if is_debug {
    println!("Message body: {:?}", message_body);
    return;
  }
  let mut slack_token = std::env::var("SLACK_TOKEN").expect("SLACK_TOKEN must be set");
  slack_token = format!("Bearer {}", &slack_token);
  
  let request = client.post("https://slack.com/api/chat.postMessage")
      .header(header::AUTHORIZATION, slack_token)
      .header(header::CONTENT_TYPE, "application/json")
      .json(message_body)
      .send();

      tokio::spawn(async move {
        match request.await {
            Ok(response) => {
                if response.status().is_success() {
                    println!("Message sent successfully");
                } else {
                    println!("Failed to send message: {:?}", response.status());
                }
            },
            Err(e) => println!("Error sending message: {:?}", e),
        }
    });
}