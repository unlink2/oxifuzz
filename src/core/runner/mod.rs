pub mod jwt;

use std::{
    io::{BufReader, BufWriter, Read, Write},
    process::{Command, Stdio},
    time::Duration,
};

use crate::core::transform::DEFAULT_USER_AGENT;

use self::jwt::{jwt_command_runner, Jwt};

use super::{
    config::{Config, HttpMethod},
    error::{Error, FResult},
    rand::Rand,
    transform::{Context, ExecRes, ExitCodes, OutputFmt, Word},
};

use log::{error, info};

/// Function that runs a command and returns an exit code and the output of the command
pub type CommandRunnerFn = fn(
    ctx: &Context,
    runner: &CommandRunnerKind,
    data: &Word,
    rand: &mut Rand,
) -> FResult<(Option<i32>, Word)>;

pub type CommandExpectFn =
    fn(ctx: &Context, exit_code: Option<i32>, data: &Word) -> FResult<ExecRes>;

#[derive(Clone)]
pub enum CommandRunnerKind {
    Shell {
        cmd: String,
        cmd_args: Vec<String>,
        cmd_arg_target: String,
        no_stdin: bool,
    },
    Http {
        url: String,
        headers: Vec<String>,
        method: HttpMethod,
        no_headers: bool,
        timeout: u32,
        cmd_arg_target: String,
    },
    Jwt(Jwt),
    Output,
    None,
}

#[derive(Clone)]
pub struct CommandRunner {
    pub(crate) kind: CommandRunnerKind,
    pub on_run: CommandRunnerFn,
    pub on_expect: CommandExpectFn,
}

impl CommandRunner {
    pub fn shell_runner(cfg: &Config) -> FResult<Option<Self>> {
        if let Some(cmd) = cfg.cmd()? {
            Ok(Some(Self {
                kind: CommandRunnerKind::Shell {
                    cmd,
                    cmd_args: cfg.cmd_args()?.unwrap_or(vec![]),
                    cmd_arg_target: cfg.exec_target.to_owned(),
                    no_stdin: cfg.no_stdin,
                },
                on_run: shell_command_runner,
                on_expect: default_command_expect,
            }))
        } else {
            error!("Command shell runner configured without a command!");
            Err(Error::InsufficientRunnerConfiguration)
        }
    }

    pub fn output_runner(_cfg: &Config) -> FResult<Option<Self>> {
        Ok(Some(Self {
            kind: CommandRunnerKind::Output,
            on_run: output_command_runner,
            on_expect: default_command_expect,
        }))
    }

    pub fn http_runner(cfg: &Config) -> FResult<Option<Self>> {
        if let Some(url) = &cfg.url {
            Ok(Some(Self {
                kind: CommandRunnerKind::Http {
                    url: url.to_owned(),
                    headers: cfg.header.to_owned(),
                    method: cfg.http_method.unwrap_or_default(),
                    no_headers: cfg.no_headers,
                    timeout: cfg.http_timeout.unwrap_or(30),
                    cmd_arg_target: cfg.exec_target.to_owned(),
                },
                on_run: http_command_runner,
                on_expect: default_command_expect,
            }))
        } else {
            error!("Command url runner configured without a url!");
            Err(Error::InsufficientRunnerConfiguration)
        }
    }

    pub fn jwt_runner(cfg: &Config) -> FResult<Option<Self>> {
        let header = if let Some(path) = &cfg.jwt_header_file {
            let mut f = std::fs::File::open(path)?;
            let mut buffer = Vec::new();
            f.read_to_end(&mut buffer)?;
            Some(String::from_utf8_lossy(&buffer).to_string())
        } else if let Some(data) = &cfg.jwt_header {
            Some(data.to_owned())
        } else {
            None
        };

        if let Some(header) = &header {
            let signature = self::jwt::Signature::from_config(cfg)?;
            Ok(Some(Self {
                kind: CommandRunnerKind::Jwt(Jwt {
                    signature,
                    header: header.to_owned(),
                    cmd_arg_target: cfg.exec_target.to_owned(),
                }),
                on_run: jwt_command_runner,
                on_expect: default_command_expect,
            }))
        } else {
            error!("Command url runner configured without a header!");
            Err(Error::InsufficientRunnerConfiguration)
        }
    }

    fn auto_select_runner(cfg: &Config) -> FResult<Option<Self>> {
        if cfg.exec.is_some() {
            Self::shell_runner(cfg)
        } else if cfg.url.is_some() {
            Self::http_runner(cfg)
        } else if cfg.jwt_header.is_some() || cfg.jwt_header_file.is_some() {
            Self::jwt_runner(cfg)
        } else {
            Self::output_runner(cfg)
        }
    }

    pub fn from_cfg(cfg: &Config) -> FResult<Option<Self>> {
        match cfg.runner {
            super::config::RunnerKindConfig::Shell => Self::shell_runner(cfg),
            super::config::RunnerKindConfig::None => Self::auto_select_runner(cfg),
            super::config::RunnerKindConfig::Output => Self::output_runner(cfg),
            super::config::RunnerKindConfig::Http => Self::http_runner(cfg),
            super::config::RunnerKindConfig::Jwt => Self::jwt_runner(cfg),
        }
    }

    pub fn run(&self, ctx: &Context, data: &Word, rand: &mut Rand) -> FResult<(Option<i32>, Word)> {
        (self.on_run)(ctx, &self.kind, data, rand)
    }

    pub fn expect(&self, ctx: &Context, exit_code: Option<i32>, data: &Word) -> FResult<ExecRes> {
        (self.on_expect)(ctx, exit_code, data)
    }

    pub fn run_and_expect(&self, ctx: &Context, data: &Word, rand: &mut Rand) -> FResult<ExecRes> {
        let (exit_code, data) = self.run(ctx, data, rand)?;
        self.expect(ctx, exit_code, &data)
    }
}

fn replace_fuzz(x: &str, cmd_arg_target: &str, ctx: &Context, rand: &mut Rand) -> FResult<String> {
    let mut x = x.to_owned();
    while x.contains(cmd_arg_target) {
        x = x.replacen(
            cmd_arg_target,
            &String::from_utf8_lossy(ctx.select_word(rand)?),
            1,
        );
    }
    Ok(x)
}

pub fn output_command_runner(
    _ctx: &Context,
    runner: &CommandRunnerKind,
    data: &Word,
    _rand: &mut Rand,
) -> FResult<(Option<i32>, Word)> {
    if let CommandRunnerKind::Output = runner {
        Ok((None, data.to_owned()))
    } else {
        Err(Error::UnsupportedCommandRunner)
    }
}

pub fn shell_command_runner(
    ctx: &Context,
    runner: &CommandRunnerKind,
    data: &Word,
    rand: &mut Rand,
) -> FResult<(Option<i32>, Word)> {
    if let CommandRunnerKind::Shell {
        cmd,
        cmd_args,
        cmd_arg_target,
        no_stdin,
    } = runner
    {
        let args: Vec<String> = cmd_args
            .iter()
            .map(|x| replace_fuzz(x, cmd_arg_target, ctx, rand))
            .try_collect()?;

        if ctx.dry_run {
            let mut output = Vec::new();
            output.write_all(cmd.as_bytes())?;
            for arg in args {
                output.write_all(b" ")?;
                output.write_all(arg.as_bytes())?;
            }
            output.write_all(data)?;
            Ok((None, output))
        } else {
            info!("Running {} {:?}", cmd, args);
            let args: Vec<&str> = args.iter().map(|x| x.as_ref()).collect();

            let mut child = Command::new(cmd)
                .args(args)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()?;

            if !no_stdin {
                let mut child_in = BufWriter::new(child.stdin.as_mut().unwrap());
                child_in.write_all(data)?;
            }
            let exit_code = child.wait()?;
            let mut child_out = BufReader::new(child.stdout.as_mut().unwrap());
            let output = std::io::read_to_string(&mut child_out)?;

            Ok((exit_code.code(), output.trim_end().into()))
        }
    } else {
        Err(Error::UnsupportedCommandRunner)
    }
}

pub fn http_command_runner(
    ctx: &Context,
    runner: &CommandRunnerKind,
    data: &Word,
    rand: &mut Rand,
) -> FResult<(Option<i32>, Word)> {
    if let CommandRunnerKind::Http {
        url,
        headers,
        method,
        no_headers,
        timeout,
        cmd_arg_target,
    } = runner
    {
        let url = replace_fuzz(url, cmd_arg_target, ctx, rand)?;

        let headers: Vec<String> = headers
            .iter()
            .map(|x| replace_fuzz(x, cmd_arg_target, ctx, rand))
            .try_collect()?;

        if ctx.dry_run {
            let mut output = Vec::new();
            output.write_all(url.as_bytes())?;
            if !headers.is_empty() {
                output.write_all(b"\n\n")?;
                for header in headers {
                    output.write_all(header.as_bytes())?;
                    output.write_all(b"\n")?;
                }
            }
            if !data.is_empty() {
                output.write_all(b"\n\n")?;
                output.write_all(data)?;
            }
            Ok((None, output))
        } else {
            use isahc::{prelude::*, Request};
            info!("Running {} {:?}", url, headers);

            let client = match method {
                HttpMethod::Get => Request::get(url),
                HttpMethod::Head => Request::head(url),
                HttpMethod::Post => Request::post(url),
                HttpMethod::Put => Request::put(url),
                HttpMethod::Delete => Request::delete(url),
            };
            let mut client = client.header("User-Agent", DEFAULT_USER_AGENT);

            for header in headers {
                let split = header.split_once(':').unwrap_or((&header, ""));
                client = client.header(split.0.to_owned(), split.1.to_owned());
            }

            let mut resp = client
                .timeout(Duration::from_millis(*timeout as u64))
                .body(data.to_owned())?
                .send()?;
            let status = resp.status();

            let mut output = Vec::new();

            if !no_headers {
                output.write_all(status.as_str().as_bytes())?;
                output.write_all(b"\n")?;
                for header in resp.headers() {
                    output.write_all(header.0.as_str().as_bytes())?;
                    output.write_all(b":")?;
                    output.write_all(header.1.as_bytes())?;
                    output.write_all(b"\n")?;
                }
                if !resp.headers().is_empty() {
                    output.write_all(b"\n\n")?;
                }
            }

            output.write_all(&resp.bytes()?)?;
            Ok((Some(status.as_u16().into()), output))
        }
    } else {
        Err(Error::UnsupportedCommandRunner)
    }
}

pub fn default_command_expect(
    ctx: &Context,
    exit_code: Option<i32>,
    data: &Word,
) -> FResult<ExecRes> {
    let success_code = if exit_code == Some(0) || exit_code.is_none() {
        ExitCodes::Success
    } else {
        ExitCodes::RunnerFailed
    };

    if ctx.expect.is_empty() {
        Ok(ExecRes {
            exit_code: success_code,
            out: data.to_owned(),
            fmt: OutputFmt::None,
        })
    } else if ctx.compare_expected(data, exit_code) {
        Ok(ExecRes {
            exit_code: success_code,
            out: data.to_owned(),
            fmt: OutputFmt::Expected,
        })
    } else {
        Ok(ExecRes {
            exit_code: ExitCodes::Failure,
            out: data.to_owned(),
            fmt: OutputFmt::NotExpected,
        })
    }
}
