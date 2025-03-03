use anyhow::{Context, Result};
use clap::Parser;
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

// 命令行参数结构
#[derive(Parser, Debug)]
#[command(version, about = "视频关键帧批量提取工具")]
struct Args {
    /// 输入目录路径
    #[arg(short, long)]
    input: String,

    /// 输出目录路径
    #[arg(short, long, default_value = "./keyframes_output")]
    output: String,

    /// 并行工作线程数
    #[arg(short, long, default_value_t = num_cpus::get())]
    threads: usize,

    /// 关键帧质量 (1-31, 1为最佳)
    #[arg(short, long, default_value_t = 2)]
    quality: u8,

    /// 文件扩展名过滤 (逗号分隔)
    #[arg(long, default_value = "mp4,mov,avi,mkv,flv")]
    extensions: String,
}

// 支持的视频格式列表
fn get_video_extensions(exts: &str) -> Vec<String> {
    exts.split(',')
        .map(|s| s.trim().to_lowercase())
        .collect()
}

fn main() -> Result<()> {
    let args = Args::parse();

    // 初始化线程池
    rayon::ThreadPoolBuilder::new()
        .num_threads(args.threads)
        .build_global()?;

    // 获取所有视频文件路径
    let video_paths: Vec<PathBuf> = WalkDir::new(&args.input)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path().is_file() && {
                let ext = e.path()
                    .extension()
                    .map(|s| s.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                get_video_extensions(&args.extensions).contains(&ext)
            }
        })
        .map(|e| e.path().to_path_buf())
        .collect();

    println!("找到 {} 个待处理视频文件", video_paths.len());

    // 并行处理视频文件
    video_paths.par_iter().try_for_each(|video_path| {
        process_video(video_path, &args.output, args.quality)
            .with_context(|| format!("处理失败: {:?}", video_path))
    })?;

    Ok(())
}

fn process_video(video_path: &Path, output_root: &str, quality: u8) -> Result<()> {
    // 创建输出目录
    let output_dir = Path::new(output_root).join(
        video_path
            .file_stem()
            .context("无效的文件名")?
            .to_string_lossy()
            .to_string(),
    );

    if output_dir.exists() {
        return Ok(());
    }

    std::fs::create_dir_all(&output_dir)
        .with_context(|| format!("创建目录失败: {:?}", output_dir))?;

    // 构建FFmpeg命令
    let output_pattern = output_dir
        .join("keyframe_%05d.jpg")
        .to_string_lossy()
        .to_string();

    let status = Command::new("ffmpeg")
        .args(&[
            "-hwaccel", "auto",         // 自动选择硬件加速
            "-i", video_path.to_str().context("无效视频路径")?,
            "-vf", "select=eq(pict_type\\,I)", // 提取I帧
            "-vsync", "vfr",
            "-q:v", &quality.to_string(), // 质量参数
            "-threads", "2",            // 每个任务线程数
            "-loglevel", "error",
            &output_pattern
        ])
        .status()
        .context("执行FFmpeg命令失败")?;

    if !status.success() {
        anyhow::bail!("FFmpeg返回错误状态: {}", status);
    }

    Ok(())
}