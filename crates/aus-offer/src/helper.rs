use std::collections::HashSet;

pub async fn list_files(path: &str) -> Result<HashSet<String>, std::io::Error> {
    let mut paths = tokio::fs::read_dir(path).await?;
    let mut plan_ids = HashSet::<String>::new();
    while let Some(entry) = paths.next_entry().await? {
        plan_ids.insert(
            entry
                .file_name()
                .into_string()
                .unwrap()
                .replace(".json", ""),
        );
    }
    Ok(plan_ids)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_list_files() {
        let res = list_files(&"/tmp/plans").await.unwrap();
        assert!(res.contains("1ST764104MS@VEC"))
    }
}
