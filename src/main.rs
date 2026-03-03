use ntex::web;
use ntex_files::Files;
use env_logger::Env;
use clap::Parser;
use ipconfig::get_adapters;
use std::num::NonZeroUsize;

#[derive(Parser, Debug)]
#[command(version, about, long_about = "这是一个高性能的静态文件服务器，支持文件列表查看和下载。\n使用示例：FileShare --port 8080")]
struct Args {
    #[arg(short, long, default_value = "files", help = "指定文件目录，默认为files。")]
    file_dir: String,

    #[arg(short, long, default_value = "/download/files", help="指定URL路径，默认为/download/files。")]
    url_path: String,

    #[arg(short, long, default_value = "info", value_parser = ["trace", "debug", "info", "warn", "error"], help="指定日志级别，默认为info。")]
    log_level: String,

    #[arg(short, long, default_value = "0.0.0.0", help = "指定主机，默认为0.0.0.0")]
    host: String,

    #[arg(short, long, default_value_t = 8080, help="指定端口，默认为8080。")]
    port: u16,

    #[arg(short, long, default_value_t = default_worker_count(), help = format!("指定工作线程数，默认为系统核心数({})。", default_worker_count()))]
    worker: usize,
}

// 默认工作线程数函数
fn default_worker_count() -> usize {
    // 使用 available_parallelism 获取系统并行度
    std::thread::available_parallelism()
        .map(NonZeroUsize::get)
        .unwrap_or(1)
}

// 参数输出
fn print_args(args: &Args) {
    println!("运行参数:");
    println!("  共享文件目录: {}", args.file_dir);
    println!("  URL路径: {}", args.url_path);
    println!("  日志级别: {}", args.log_level);
    println!("  主机: {}", args.host);
    println!("  端口: {}", args.port);
    println!("  工作线程数: {}", args.worker);
}

/// 获取所有局域网 IP 地址
fn get_all_local_ips() -> Vec<String> {
    let mut ips = Vec::new();

    match get_adapters() {
        Ok(adapters) => {
            for adapter in adapters {
                // 只考虑已启用的网络适配器
                if adapter.oper_status() == ipconfig::OperStatus::IfOperStatusUp {

                    // 获取所有 IPv4 地址
                    for ip in adapter.ip_addresses() {
                        if ip.is_ipv4() && !ip.is_loopback() {
                            let ip_str = ip.to_string();
                            if is_private_ip(&ip_str) {
                                ips.push(ip_str);
                            }
                        }
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("获取网络适配器失败: {}", e);
        }
    }

    if ips.is_empty() {
        ips.push("127.0.0.1".to_string());
    }

    ips
}

/// 判断是否为私有 IP 地址（局域网）
fn is_private_ip(ip: &str) -> bool {
    ip.starts_with("10.") ||
        ip.starts_with("172.16.") || ip.starts_with("172.17.") ||
        ip.starts_with("172.18.") || ip.starts_with("172.19.") ||
        ip.starts_with("172.20.") || ip.starts_with("172.21.") ||
        ip.starts_with("172.22.") || ip.starts_with("172.23.") ||
        ip.starts_with("172.24.") || ip.starts_with("172.25.") ||
        ip.starts_with("172.26.") || ip.starts_with("172.27.") ||
        ip.starts_with("172.28.") || ip.starts_with("172.29.") ||
        ip.starts_with("172.30.") || ip.starts_with("172.31.") ||
        ip.starts_with("192.168.")
}

// 绝对路径
fn get_absolute_path(path_str: &str) -> std::path::PathBuf {
    let path = std::path::Path::new(path_str);

    if path.is_absolute() {
        // 如果已经是绝对路径，直接返回
        path.to_path_buf()
    } else {
        // 否则相对于当前目录
        match std::env::current_dir() {
            Ok(current_dir) => current_dir.join(path),
            Err(_) => {
                log::error!("无法获取当前目录，使用相对路径");
                path.to_path_buf()
            }
        }
    }
}

#[ntex::main]
async fn main() -> std::io::Result<()>{
    let args = Args::parse();

    print_args(&args);

    // 设置环境变量来启用日志
    env_logger::init_from_env(Env::default().default_filter_or(&args.log_level));

    // 目录不存在就创建
    if !std::path::Path::new(&args.file_dir).exists() {
        log::warn!("目录 {} 不存在，正在创建...", &args.file_dir);
        std::fs::create_dir_all(&args.file_dir)
            .unwrap_or_else(|e| panic!("创建目录 {} 失败: {} (当前目录: {})", &args.file_dir, e, std::env::current_dir().unwrap_or_default().display()));
        log::info!("创建目录 {} 成功", &args.file_dir);
    }

    // 输出访问路径
    log::info!("共享文件夹绝对路径：{}", get_absolute_path(&args.file_dir).display());
    log::info!("本机访问地址：http://127.0.0.1:{}{}", args.port, &args.url_path);
    for ip in get_all_local_ips() {
        log::info!("局域网访问地址：http://{}:{}{}", ip, args.port, &args.url_path);
    }

    // 在 move 闭包之前克隆需要的值
    let url_path = args.url_path.clone();
    let file_dir = args.file_dir.clone();
    let host = args.host.clone();
    let port = args.port;
    let worker = args.worker;

    web::HttpServer::new(move || {
        web::App::new()
            .wrap(web::middleware::Logger::default())
            .service(
                Files::new(&url_path, &file_dir)
                    .show_files_listing()
                    .disable_content_disposition(),
            )
    })
    .workers(worker)
    .bind((host, port))?
    .run()
    .await
}
