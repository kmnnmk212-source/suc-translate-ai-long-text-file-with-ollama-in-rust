
use clap::Parser;
use reqwest::Client;
use serde_json::json;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::time::Duration;
use tokio::time::sleep;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    input: String,
    #[arg(short, long)]
    output: String,
    #[arg(short, long, default_value = "aya:8b")] 
    model: String, 
    #[arg(long, default_value = "ar")]
    output_language: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    
    if !std::path::Path::new(&args.input).exists() {
        eprintln!("Error: Input file not found.");
        return Ok(());
    }

    let content = fs::read_to_string(&args.input).expect("Failed to read file");
    fs::write(&args.output, "")?; 

    // تعديل 1: تقليل حجم القطعة إلى 700 حرف لتخفيف الحمل
    let chunks = split_text_smartly(&content, 700); 
    let total_chunks = chunks.len();

    println!("Model: {}, Splitting into {} chunks...", args.model, total_chunks);

    // تعديل 2: زيادة مهلة الانتظار إلى 600 ثانية (10 دقائق)
    let client = Client::builder()
        .timeout(Duration::from_secs(600))
        .build()?;

    for (i, chunk) in chunks.iter().enumerate() {
        println!("Translating chunk {}/{} ({} chars)...", i + 1, total_chunks, chunk.len());
        
        let translated = translate_chunk(&client, chunk, &args.output_language, &args.model).await;
        
        let mut file = OpenOptions::new().write(true).append(true).open(&args.output)?;
        writeln!(file, "{}\n", translated)?;
    }
    println!("✅ Done! Saved to {}", args.output);
    Ok(())
}

fn split_text_smartly(text: &str, max_len: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if current.len() + word.len() > max_len {
            chunks.push(current.trim().to_string());
            current = String::new();
        }
        current.push_str(word);
        current.push(' ');
    }
    if !current.trim().is_empty() { chunks.push(current.trim().to_string()); }
    chunks
}

async fn translate_chunk(client: &Client, text: &str, lang: &str, model: &str) -> String {
    let prompt = format!("Translate the following text into Arabic ({}). Provide ONLY the translation. Text:\n{}", lang, text);
    
    let mut attempts = 0;
    while attempts < 5 { 
        let res = client.post("http://localhost:11434/api/generate")
            .json(&json!({ 
                "model": model, 
                "prompt": prompt, 
                "stream": false,
                // تعديل 3: زيادة نافذة السياق لمنع الامتلاء
                "options": { "num_ctx": 8192, "temperature": 0.3 }
            }))
            .send().await;

        if let Ok(resp) = res {
            if let Ok(txt) = resp.text().await {
                if let Ok(j) = serde_json::from_str::<serde_json::Value>(&txt) {
                    if let Some(out) = j.get("response") { return out.as_str().unwrap_or("").to_string(); }
                }
            }
        }
        eprintln!("⚠️ Retry {}/5 due to error or timeout...", attempts + 1);
        sleep(Duration::from_secs(5)).await;
        attempts += 1;
    }
    format!("[FAILED CHUNK: Server overloaded]") 
}
