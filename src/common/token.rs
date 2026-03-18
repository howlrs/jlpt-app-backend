use axum_extra::extract::cookie::{Cookie, SameSite};

pub fn build_refresh_cookie(raw_token: &str) -> Cookie<'static> {
    let is_production = std::env::var("FRONTEND_URL")
        .map(|u| u.starts_with("https://"))
        .unwrap_or(false);

    let mut cookie = Cookie::build(("refresh_token", raw_token.to_string()))
        .http_only(true)
        .same_site(SameSite::Lax)
        .path("/api/auth/refresh")
        .max_age(time::Duration::days(14));

    if is_production {
        cookie = cookie.secure(true);
    }

    cookie.build()
}

pub fn build_clear_refresh_cookie() -> Cookie<'static> {
    Cookie::build(("refresh_token", ""))
        .http_only(true)
        .same_site(SameSite::Lax)
        .path("/api/auth/refresh")
        .max_age(time::Duration::seconds(0))
        .build()
}
