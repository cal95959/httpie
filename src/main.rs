
use anyhow::{anyhow, Result};
use clap::Parser;
use colored::Colorize;
use mime::Mime;
use reqwest::{header, Client, Response, Url};
use std::{collections::HashMap,str::FromStr};
use syntect::{
    easy::HighlightLines,
    parsing::SyntaxSet,
    highlighting::{Style, ThemeSet},
    util::{as_24_bit_terminal_escaped, LinesWithEndings}
};

// 以下部分用于处理CLI

//定义httpie的CLI主入口， 它包含若干个子命令
// 下面的///注释是文档， clap会将其作为CLI的帮助

/// A native httpie implementation with Rust, can you imagine how easy it is ?

#[derive(Parser, Debug)]
#[clap(version = "1.0", author = "cal")]
struct Opts{
    #[clap(subcommand)]
    subcmd: SubCommand,
}

// 子命令分别对应不同的HTTP方法，目前只支持get/post
#[derive(Parser, Debug)]
enum SubCommand {
    Get(Get),
    Post(Post),
    // 暂且不支持其他HTTP方法
}

// get子命令

/// feed get with an url and will retrieve the response for you
#[derive(Parser, Debug)]
struct Get {
    /// HTTP请求的URL
    #[clap(parse(try_from_str = parse_url))]
    url: String
}

// post 子命令。 需要输入一个url

/// feed post with an url and optional key=value pairs. We will post the data
/// as JSON, and retrieve the response for you
#[derive(Parser, Debug)]
struct Post {
    /// HTTP请求的URL
    #[clap(parse(try_from_str = parse_url))]
    url: String,
    // HTTP请求的body
    #[clap(parse(try_from_str = parse_kv_pair))]
    body: Vec<KvPair>,
}

/// 命令中的key=value可以通过parse_kv_pair解析成KvPair结构
#[derive(Debug, PartialEq)]
struct KvPair {
    k: String,
    v: String,
}

/// 当实现FromStr trait后，可以用str.parse()方法将字符串解析成KvPair
impl FromStr for KvPair {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // 使用=进行split，这会得到一个迭代器
        let mut split = s.split('=');
        let err = || anyhow!(format!("Failed to parse {}", s));
        Ok(Self {
            // 从迭代器中取第一个结果作为key, 迭代器返回Some(T)/None
            // 将其转换成Ok(T)/Err(E),然后用？处理错误
            k: (split.next().ok_or_else(err)?).to_string(),
            // 从迭代器中取第二个结果作为value
            v: (split.next().ok_or_else(err)?).to_string(),
        })
    }
}

// 因为我们为KvPair实现了FromStr, 这里可以直接s.parse()得到KvPair
fn parse_kv_pair(s: &str) -> Result<KvPair> {
    s.parse()
}

fn parse_url(s: &str) -> Result<String> {
    // 这里我们仅仅检查以下URL是否合法
    let _url: Url = s.parse()?;
    Ok(s.into())
}

/// 处理get子命令
// async fn get(client: Client, args: &Get) -> Result<()> {
//     let resp = client.get(&args.url).send().await?;
//     Ok(print_resp(resp).await?)
// }


// fn main() {
//     let opts: Opts = Opts::parse();
//     println!("{:?}", opts)
// }

async fn get(client: Client, args: &Get) -> Result<()> {
    let resp = client.get(&args.url).send().await?;
    // println!("{:?}", resp.text().await?);

    Ok(print_resp(resp).await?)
}

async fn post(client: Client, args: &Post) -> Result<()> {
    let mut body = HashMap::new();
    for pair in args.body.iter() {
        body.insert(&pair.k, &pair.v);
    }
    let resp = client.post(&args.url).json(&body).send().await?;
    // println!("{:?}", resp.text().await?);

    Ok(print_resp(resp).await?)
}

#[tokio::main]
async fn main() -> Result<()> {
    let opts: Opts = Opts::parse();
    // 生成一个HTTP客户端
    let client = Client::new();
    let result = match opts.subcmd {
        SubCommand::Get(ref args) => get(client, args).await?,
        SubCommand::Post(ref args) => post(client, args).await?,
    };

    Ok(result)
}

// 打印服务器版本号 + 状态码
fn print_status(resp: &Response) {
    let status = format!("{:?} {}", resp.version(), resp.status()).blue();
    println!("{}\n", status);
}

// 打印服务器返回的HTTP header
fn print_headers(resp: &Response) {
    for (name, value) in resp.headers() {
        println!("{}: {:?}", name.to_string().green(), value);
    }
    print!("\n");
}

/// 打印服务器返回的HTTP body
fn print_body(m: Option<Mime>, body: &String) {
    match m {
        // 对于"application/json", 我们pretty print
        // Some(v) if v == mime::APPLICATION_JSON => {
        //     println!("{}", jsonxf::pretty_print(body).unwrap().cyan());
        // }
        Some(v) if v == mime::APPLICATION_JSON => print_syntect(body, "json"),
        Some(v) if v == mime::TEXT_HTML => print_syntect(body, "html"),
        // 其它 mime type, 直接输出
        _ => println!("{}", body)
    }
}

/// 打印整个响应
async fn print_resp(resp: Response) -> Result<()> {
    print_status(&resp);
    print_headers(&resp);
    let mime = get_content_type(&resp);
    let body = resp.text().await?;
    print_body(mime, &body);
    
    Ok(())
}

/// 将服务器返回的 content type解析成Mime类型
fn get_content_type(resp: &Response) -> Option<Mime> {
    resp.headers()
        .get(header::CONTENT_TYPE)
        .map(|v| v.to_str().unwrap().parse().unwrap())
}

fn print_syntect(s: &str, ext: &str) {
    // load these once at the start of your program
    let ps = SyntaxSet::load_defaults_newlines();
    let ts = ThemeSet::load_defaults();
    let syntax = ps.find_syntax_by_extension(ext).unwrap();
    let mut h = HighlightLines::new(syntax, &ts.themes["base16-ocean.dark"]);
    for line in LinesWithEndings::from(s) {
        let ranges: Vec<(Style, &str)> = h.highlight(line, &ps);
        let escaped = as_24_bit_terminal_escaped(&ranges[..], true);
        print!("{}", escaped);
    }
}


// 仅在cargo test时才编译
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn parse_url_works() {
        assert!(parse_url("abc").is_err());
        assert!(parse_url("http://abc.xyz").is_ok());
        assert!(parse_url("https://httpbin.org/post").is_ok());
    }
    
    #[test]
    fn parse_kv_pair_works() {
        assert!(parse_kv_pair("a").is_err());
        assert_eq!(
            parse_kv_pair("a=1").unwrap(),
            KvPair {
                k: "a".into(),
                v: "1".into(),
            }
        );
        
        assert_eq!(
            parse_kv_pair("b=").unwrap(),
            KvPair {
                k: "b".into(),
                v: "".into(),
            }
        );
    }
}





















