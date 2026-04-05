use clap::Parser;
use reqwest::Client;
use serde_json::Value;
use anyhow::{Context, Result};

/// NCBI 基因信息查询工具
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// 基因名称或 NCBI Gene ID (例如: TP53, 7157)
    query: String,

    /// 以 JSON 格式输出原始响应
    #[arg(long)]
    json: bool,
    #[arg(short, long, default_value = "human")]
    species: String,
}

#[derive(Debug)]
struct GeneInfo {
    uid: String,
    name: String,
    description: String,
    chromosome: String,
    map_location: String,
    summary: String,
    other_aliases: String,
}

impl GeneInfo {
    fn display(&self) {
        println!("Gene Information:");
        println!("  NCBI Gene ID: {}", self.uid);
        println!("  Symbol:       {}", self.name);
        println!("  Description:  {}", self.description);
        println!("  Chromosome:   {}", self.chromosome);
        println!("  Map Location: {}", self.map_location);
        println!("  Aliases:      {}", self.other_aliases);
        println!("  Summary:      {}", self.summary);
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let taxid = match args.species.to_lowercase().as_str() {
    "human" | "homo sapiens" => "9606",
    "mouse" | "mus musculus" => "10090",
    "rat" | "rattus norvegicus" => "10116",
    // 如果是纯数字，则假定为TaxID
    _ if args.species.parse::<u32>().is_ok() => &args.species,
    _ => {
        eprintln!("Warning: Unknown species '{}', searching all species.", args.species);
        "" // 如果未知，则不添加过滤
    }
};
    let client = Client::builder()
        .user_agent("gene/0.1.0 szoloa@hotmail.com") // 请替换为你的邮箱
        .build()?;

    // 1. 通过 esearch 获取 Gene ID
    let search_url = "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/esearch.fcgi";
    let search_term = if !taxid.is_empty() {
            format!("{}[Gene Name] AND {}[Taxonomy ID]", args.query, taxid)
        } else {
            args.query.clone()
        };
    let search_resp = client
        .get(search_url)
        .query(&[
            ("db", "gene"),
            ("term", &search_term),
            ("retmode", "json"),
        ])
        .send()
        .await
        .context("Failed to send esearch request")?;

    let search_json: Value = search_resp.json().await.context("Failed to parse esearch JSON")?;
    let id_list = search_json["esearchresult"]["idlist"]
        .as_array()
        .context("Invalid esearch response: missing idlist")?;

    if id_list.is_empty() {
        anyhow::bail!("No gene found for query '{}'", args.query);
    }

    let gene_id = id_list[0].as_str().unwrap();
    if id_list.len() > 1 {
        eprintln!(
            "Note: Found {} results, using the first one (ID: {})",
            id_list.len(),
            gene_id
        );
    }

    // 2. 通过 esummary 获取详细信息
    let summary_url = "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/esummary.fcgi";
    let summary_resp = client
        .get(summary_url)
        .query(&[("db", "gene"), ("id", gene_id), ("retmode", "json")])
        .send()
        .await
        .context("Failed to send esummary request")?;

    let summary_json: Value = summary_resp.json().await.context("Failed to parse esummary JSON")?;

    if args.json {
        // 输出原始 JSON
        println!("{}", serde_json::to_string_pretty(&summary_json)?);
        return Ok(());
    }

    // 提取基因信息
    let gene_data = &summary_json["result"][gene_id];
    let gene_info = GeneInfo {
        uid: gene_id.to_string(),
        name: gene_data["name"].as_str().unwrap_or("N/A").to_string(),
        description: gene_data["description"].as_str().unwrap_or("N/A").to_string(),
        chromosome: gene_data["chromosome"].as_str().unwrap_or("N/A").to_string(),
        map_location: gene_data["map_location"].as_str().unwrap_or("N/A").to_string(),
        summary: gene_data["summary"].as_str().unwrap_or("No summary available").to_string(),
        other_aliases: gene_data["other_aliases"].as_str().unwrap_or("None").to_string(),
    };

    gene_info.display();
    Ok(())
}
