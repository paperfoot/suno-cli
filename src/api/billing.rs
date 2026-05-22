use super::SunoClient;
use super::types::BillingInfo;
use crate::errors::CliError;

impl SunoClient {
    pub async fn billing_info(&self) -> Result<BillingInfo, CliError> {
        self.with_auth_retry(|| async {
            let resp = self.get("/api/billing/info/").send().await?;
            let resp = self.check_response(resp).await?;
            let raw = resp.text().await?;
            let mut info: BillingInfo = serde_json::from_str(&raw).map_err(|e| CliError::Api {
                code: "billing_schema_drift",
                message: format!(
                    "billing/info returned unexpected JSON/body ({e}): {}",
                    raw.replace(['\n', '\r'], " ")
                        .chars()
                        .take(500)
                        .collect::<String>()
                ),
            })?;
            if info.total_credits_left == 0 {
                info.total_credits_left = info.credits;
            }
            if info.plan.name.is_empty() {
                info.plan.name = if info.is_active {
                    "Active".to_string()
                } else {
                    "Unknown".to_string()
                };
            }
            if info.plan.plan_key.is_empty() {
                info.plan.plan_key = info.plan.name.to_ascii_lowercase();
            }
            Ok(info)
        })
        .await
    }
}
