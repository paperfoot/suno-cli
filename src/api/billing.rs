use super::SunoClient;
use super::types::BillingInfo;
use crate::errors::CliError;

impl SunoClient {
    pub async fn billing_info(&self) -> Result<BillingInfo, CliError> {
        self.with_auth_retry(|| async {
            let resp = self.get("/api/billing/info/").send().await?;
            let resp = self.check_response(resp).await?;
            super::json_response::decode(resp, "Suno billing").await
        })
        .await
    }
}
