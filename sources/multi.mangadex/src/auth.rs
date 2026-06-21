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

const AUTH_URL: &str = "https://auth.mangadex.org";

fn request_token(body: String) -> Result<String> {
	let url = format!("{AUTH_URL}/realms/mangadex/protocol/openid-connect/token");
	let response = Request::post(url)?
		.header("Content-Type", "application/x-www-form-urlencoded")
		.body(body)
		.data()?;

	let Ok(string_value) = String::from_utf8(response) else {
		return Err(AidokuError::Message("Failed to read token response".into()));
	};

	let token_response = serde_json::from_str::<TokenResponse>(&string_value)
		.map_err(|e| AidokuError::JsonParseError(Rc::new(e)))?;

	let Some(access_token) = token_response.access_token else {
		return Err(AidokuError::Message("Missing access token".into()));
	};

	settings::set_access_token(&access_token);
	if let Some(refresh_token) = token_response.refresh_token {
		settings::set_refresh_token(&refresh_token);
	}

	Ok(access_token)
}

// Refresh if we can, otherwise log in with username/password.
fn login() -> Result<String> {
	let Some(client_id) = settings::get_client_id() else {
		return Err(AidokuError::Message("Missing client ID".into()));
	};
	let client_secret = settings::get_client_secret().unwrap_or_default();

	if let Some(refresh_token) = settings::get_refresh_token() {
		let body = format!(
			"grant_type=refresh_token\
				&client_id={client_id}\
				&client_secret={client_secret}\
				&refresh_token={refresh_token}",
		);
		if let Ok(access_token) = request_token(body) {
			return Ok(access_token);
		}
	}

	let Some(username) = settings::get_username() else {
		return Err(AidokuError::Message("Missing username".into()));
	};
	let Some(password) = settings::get_password() else {
		return Err(AidokuError::Message("Missing password".into()));
	};
	let body = format!(
		"grant_type=password\
			&client_id={client_id}\
			&client_secret={client_secret}\
			&username={username}\
			&password={password}",
	);
	request_token(body)
}

pub trait AuthedRequest {
	fn authed_send(self) -> Result<Response>;
}

impl AuthedRequest for Request {
	fn authed_send(mut self) -> Result<Response> {
		let access_token = match settings::get_access_token() {
			Some(access_token) => access_token,
			None => login()?,
		};

		self.set_header("Authorization", &format!("Bearer {access_token}"));

		let mut response = self.send()?;

		if response.status_code() == 401 {
			let access_token = login()?;
			response = response
				.into_request()
				.header("Authorization", &format!("Bearer {access_token}"))
				.send()?;
		}

		Ok(response)
	}
}
