use anyhow::Result;
use reqwest;
use rmcp::{
    model::{ServerCapabilities, ServerInfo}, schemars,
    tool,
    transport::stdio, ServerHandler,
    ServiceExt,
};
use serde;
use tracing_subscriber::{self, EnvFilter};
use tracing_appender::rolling::{RollingFileAppender, Rotation};

const NWS_API_BASE: &str = "https://restapi.amap.com/v3/weather/weatherInfo?parameters";
const USE_AGENT: &str = "weather-app/1.0";
const BIND_ADDRESS: &str = "127.0.0.1:8000";


#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct AlertResponse {
    pub status: String,
    pub count: String,
    pub info: String,
    pub infocode: String,
    pub lives: Vec<Live>,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct Live {
    pub province: String,
    pub city: String,
    pub adcode: String,
    pub weather: String,
    pub temperature: String,
    pub winddirection: String,
    pub windpower: String,
    pub humidity: String,
    pub reporttime: String,
    pub temperature_float: String,
    pub humidity_float: String,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct PointsResponse {
    pub status: String,
    pub count: String,
    pub info: String,
    pub infocode: String,
    pub forecasts: Vec<Forecast>,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct Forecast {
    pub city: String,
    pub casts: Vec<DayForecast>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct DayForecast {
    pub date: String,
    pub dayweather: String,
    pub nightweather: String,
    pub daytemp: String,
    pub nighttemp: String,
    pub daywind: String,
    pub nightwind: String,
    pub daypower: String,
    pub nightpower: String,
}
fn format_alerts(alerts: &[Live]) -> String {
    if alerts.is_empty() {
        return "No active alerts found.".to_string();
    }
    let mut result = String::with_capacity(alerts.len() * 200);

    for alert in alerts {
        result.push_str(&format!(
            "省份: {}\n城市: {}\n天气: {}\n温度: {}°\n风向: {}({})\n---\n",
            alert.province,
            alert.city,
            alert.weather,
            alert.temperature,
            alert.winddirection,
            alert.windpower
        ));
    }
    result
}

fn format_forecast(periods: &[Forecast]) -> String {
    if periods.is_empty() {
        return "No forecast data available.".to_string();
    }
    let mut result = String::with_capacity(150 * periods.len());

    for period in periods {
        for day in &period.casts {
            result.push_str(&format!(
                "日期: {}\n白天: {} {}° {}({}) \n夜间: {} {}° {}({})\n---\n",
                day.date,
                day.dayweather, day.daytemp, day.daywind, day.daypower,
                day.nightweather, day.nighttemp, day.nightwind, day.nightpower
            ));
        }
    }
    result
}
#[derive(Debug, Clone)]
pub struct Weather {
    client: reqwest::Client,
}
#[tool(tool_box)]
impl Weather {
    #[allow(dead_code)]
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent(USE_AGENT)
            .build()
            .expect("Failed to create HTTP client");
        Self { client }
    }
    //key 3e7f6bcddfcbe0f1619f5842c9226908
    async fn make_request<T>(&self, url: &str) -> Result<T, String>
    where
        T: serde::de::DeserializeOwned,
    {
        tracing::info!("Making request to {}", url);

        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| format!("Failed to make request to {}: {}", url, e))?;

        tracing::info!("Received response: {:?}", response);

        match response.status() {
            reqwest::StatusCode::OK => response
                .json::<T>()
                .await
                .map_err(|e| format!("Failed to parse request to {}: {}", url, e)),
            status => Err(format!("Failed to make request to {}: {}", url, status)),
        }
    }
    #[tool(description = "获取当天，天气情况")]
    async fn get_alerts(
        &self,
        #[tool(param)]
        #[schemars(description = "城市编码")]
        state: String,
    ) -> String {
        tracing::info!("Received request for weather alerts in state: {}", state);
        let url = format!(
            "{}&key=3e7f6bcddfcbe0f1619f5842c9226908&city={}&output=json",
            NWS_API_BASE, state
        );
        let result = self.make_request::<AlertResponse>(&url).await;
        match result {
            Ok(alerts) => format_alerts(&alerts.lives),
            Err(e) => {
                tracing::error!("Failed to fetch alerts: {}", e);
                "No alerts found or an error occurred.".to_string()
            }
        }
    }

    #[tool(description = "获取最近几天，天气预报")]
    async fn get_forecast(
        &self,
        #[tool(param)]
        #[schemars(description = "城市编码")]
        city: String,
    ) -> String {
        tracing::info!("Received request for forecast with city code {}", city,);

        let url = format!(
            "{}&key=3e7f6bcddfcbe0f1619f5842c9226908&city={}&output=json&extensions=all",
            NWS_API_BASE, city
        );
        println!("111111 {}", url);
        let points_result = self.make_request::<PointsResponse>(&url).await;

        match points_result {
            Ok(points) => format_forecast(&points.forecasts),
            Err(e) => {
                tracing::error!("Failed to fetch points: {}", e);
                return "No forecast found or an error occurred.".to_string();
            }
        }
    }
}

#[tool(tool_box)]
impl ServerHandler for Weather {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some("A simple weather forecaster".into()),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging with explicit configuration
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let file_appender = RollingFileAppender::new(
        Rotation::DAILY,
        "./logs",
        "app.log"
    );
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_writer(file_appender)
        .with_ansi(false)  // Enable ANSI colors for better visibility
        .with_target(false)  // Disable target for cleaner output
        .init();

    //
    tracing::info!("Starting weather MCP server");

    let service = Weather::new().serve(stdio()).await.inspect_err(|e| {
        tracing::error!("serving error: {:?}", e);
    })?;

    service.waiting().await?;

    Ok(())
}
