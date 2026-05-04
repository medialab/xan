fn tokenize_pipeline(input: &str) -> Result<Vec<String>, String> {
    let raw = shlex::split(input).ok_or_else(|| format!("could not parse pipeline: {}", input))?;

    let mut tokens = Vec::with_capacity(raw.len());

    // NOTE: renormalizing tokens around pipes (e.g. when given a pipe
    // that is not separated by a space `progress |search -es Category`).
    for token in raw.into_iter() {
        if token == "|" {
            tokens.push(token);
        } else if let Some(rest) = token.strip_prefix("|") {
            tokens.push("|".to_string());
            tokens.push(rest.trim().to_string());
        } else if let Some(rest) = token.strip_suffix("|") {
            tokens.push(rest.trim().to_string());
            tokens.push("|".to_string());
        } else {
            tokens.push(token);
        }
    }

    Ok(tokens)
}

pub fn parse_pipeline(input: &str) -> Result<Vec<Vec<String>>, String> {
    let tokens = tokenize_pipeline(input)?;

    Ok(tokens
        .split(|token| token == "|")
        .map(|args| {
            if args.first().map(|arg| arg.as_str()) == Some("xan") {
                args[1..].to_vec()
            } else {
                args.to_vec()
            }
        })
        .collect())
}
