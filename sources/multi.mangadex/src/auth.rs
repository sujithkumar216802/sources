use crate::models::TokenResponse;
use crate::settings;
use aidoku::{
	Result,
	alloc::{String, rc::Rc},
	imports::{
		error::AidokuError,
		net::{Request, Response},
	},
	prelude::*,
};

const CLIENT_ID: &str = "personal-client-243b0512-fa6f-42bd-8d9e-1e5367a64ea0-dc367f79";
const AUTH_URL: &str = "https://auth.mangadex.org";

fn refresh_access_token() -> Result<TokenResponse> {
	let Ok(token_response) = settings::get_token() else {
		settings::clear_token();
		return Err(AidokuError::Message("Not logged in".into()));
	};

	let Some(refresh_token) = token_response.refresh_token else {
		settings::clear_token();
		return Err(AidokuError::Message("Missing refresh token".into()));
	};

	let url = format!("{AUTH_URL}/realms/mangadex/protocol/openid-connect/token");
	let client_secret = settings::get_client_secret().unwrap_or_default();
	let body = format!(
		"client_id={CLIENT_ID}\
			&grant_type=refresh_token\
			&refresh_token={refresh_token}\
			&client_secret={client_secret}",
	);
	let token_response = Request::post(url)?
		.header("Content-Type", "application/x-www-form-urlencoded")
		.body(body)
		.data()?;

	settings::clear_token();

	let Ok(string_value) = String::from_utf8(token_response) else {
		return Err(AidokuError::Message("Failed to refresh token".into()));
	};

	let token_response = serde_json::from_str::<TokenResponse>(&string_value)
		.map_err(|e| AidokuError::JsonParseError(Rc::new(e)))?;

	settings::set_token(&string_value);

	Ok(token_response)
}

pub trait AuthedRequest {
	fn authed_send(self) -> Result<Response>;
}

impl AuthedRequest for Request {
	fn authed_send(mut self) -> Result<Response> {
		let token_response = settings::get_token()?;
		let Some(access_token) = token_response.access_token else {
			settings::clear_token();
			return Err(AidokuError::Message("Missing access token".into()));
		};

		self.set_header("Authorization", &format!("Bearer {access_token}"));

		let mut response = self.send()?;
		let status = response.status_code();

		if status == 401 {
			let token_response = refresh_access_token()?;
			let Some(access_token) = token_response.access_token else {
				settings::clear_token();
				return Err(AidokuError::Message("Missing access token".into()));
			};
			response = response
				.into_request()
				.header("Authorization", &format!("Bearer {access_token}"))
				.send()?;
		}

		Ok(response)
	}
}
