use serde::Serialize;
use std::env;

#[derive(Serialize)]
struct SearchResult {
    title: String,
    url: String,
    snippet: String,
}

#[derive(Serialize)]
struct SearchResponse {
    query: String,
    results: Vec<SearchResult>,
}

fn search_ddg(query: &str, count: usize) -> Result<SearchResponse, Box<dyn std::error::Error>> {
    let url = format!("https://html.duckduckgo.com/html/?q={}", ureq::utils::encode(query));
    let body = ureq::get(&url)
        .set("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) moltis-search/0.1")
        .call()?
        .into_string()?;

    let mut results = Vec::new();
    for chunk in body.split("class=\"result__a\"").skip(1).take(count) {
        let url = extract_between(chunk, "href=\"", "\"").unwrap_or_default();
        let title = extract_between(chunk, ">", "</a>").unwrap_or_default();
        let snippet = body
            .split(&url)
            .nth(1)
            .and_then(|s| extract_between(s, "class=\"result__snippet\">", "</"))
            .unwrap_or_default();

        if !url.is_empty() {
            results.push(SearchResult {
                title: html_decode(&title),
                url: url.to_string(),
                snippet: html_decode(&snippet),
            });
        }
    }

    Ok(SearchResponse { query: query.to_string(), results })
}

fn extract_between<'a>(s: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let i = s.find(start)? + start.len();
    let j = s[i..].find(end)? + i;
    Some(&s[i..j])
}

fn html_decode(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("<b>", "")
        .replace("</b>", "")
        .trim()
        .to_string()
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: moltis-search <query> [count]");
        std::process::exit(1);
    }
    let query = &args[1];
    let count: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(5);

    match search_ddg(query, count) {
        Ok(resp) => println!("{}", serde_json::to_string_pretty(&resp).unwrap()),
        Err(e) => {
            let err = serde_json::json!({"error": e.to_string()});
            println!("{err}");
            std::process::exit(1);
        }
    }
}
