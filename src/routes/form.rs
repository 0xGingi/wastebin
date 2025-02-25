use crate::db::InsertEntry;
use crate::id::Id;
use crate::{pages, AppState, Error};
use axum::extract::{Form, State};
use axum::response::Redirect;
use axum_extra::extract::cookie::{Cookie, SignedCookieJar};
use rand::Rng;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Entry {
    pub text: String,
    pub extension: Option<String>,
    pub expires: String,
}

impl From<Entry> for InsertEntry {
    fn from(entry: Entry) -> Self {
        let burn_after_reading = Some(entry.expires == "burn");

        let expires = match entry.expires.parse::<u32>() {
            Ok(0) | Err(_) => None,
            Ok(secs) => Some(secs),
        };

        Self {
            text: entry.text,
            extension: entry.extension,
            expires,
            burn_after_reading,
            uid: None,
        }
    }
}

pub async fn insert(
    state: State<AppState>,
    jar: SignedCookieJar,
    Form(entry): Form<Entry>,
) -> Result<(SignedCookieJar, Redirect), pages::ErrorResponse<'static>> {
    let id: Id = tokio::task::spawn_blocking(|| {
        let mut rng = rand::thread_rng();
        rng.gen::<u32>()
    })
    .await
    .map_err(Error::from)?
    .into();

    // Retrieve uid from cookie or generate a new one.
    let uid = if let Some(cookie) = jar.get("uid") {
        cookie
            .value()
            .parse::<i64>()
            .map_err(|err| Error::CookieParsing(err.to_string()))?
    } else {
        state.db.next_uid().await?
    };

    let mut entry: InsertEntry = entry.into();
    entry.uid = Some(uid);

    let url = id.to_url_path(&entry);
    let burn_after_reading = entry.burn_after_reading.unwrap_or(false);

    state.db.insert(id, entry).await?;

    let jar = jar.add(Cookie::new("uid", uid.to_string()));

    if burn_after_reading {
        Ok((jar, Redirect::to(&format!("/burn{url}"))))
    } else {
        Ok((jar, Redirect::to(&url)))
    }
}
