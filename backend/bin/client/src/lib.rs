mod channel;
mod channel_list;
mod conn;
mod constants;
mod event;
mod handler;

use handler::handle_event;
use kiwi_talk_api::auth::CredentialState;
use parking_lot::RwLock;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{sync::Arc, task::Poll};
use tauri::{
    generate_handler,
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};

use anyhow::{anyhow, Context};
use futures::future::poll_fn;
use headless_talk::{
    init::{config::ClientEnv, Credential, TalkInitializer},
    ClientStatus, HeadlessTalk,
};
use talk_loco_client::futures_loco_protocol::LocoClient;
use tokio::sync::mpsc;

use kiwi_talk_result::TauriResult;
use kiwi_talk_system::get_system_info;

use conn::checkin;
use constants::{TALK_APP_VERSION, TALK_MCCMNC, TALK_NET_TYPE, TALK_OS};

use self::{conn::create_secure_stream, event::ClientEvent};

// 定义异步函数，用于初始化插件
pub async fn init<R: Runtime>(name: &'static str) -> anyhow::Result<TauriPlugin<R>> {
    Ok(Builder::new(name)
        // 管理插件的状态
        .setup(move |handle| {
            handle.manage::<Client>(Client::new());

            Ok(())
        })
        .invoke_handler(generate_handler![
            created,
            create,
            destroy,
            next_event,
            channel_list::channel_list,
            channel::load_channel,
            channel::channel_send_text,
            channel::channel_load_chat,
            channel::normal::normal_channel_read_chat,
        ])
        .build())
}

// 定义插件状态类型的别名
type ClientState<'a> = tauri::State<'a, Client>;

// 定义命令，用于检查插件是否已
#[tauri::command]
fn created(state: ClientState<'_>) -> bool {
    state.created()
}

// 定义枚举，表示客户端状态
#[derive(Clone, Deserialize, Copy)]
enum Status {
    Unlocked,
    Locked,
}

// 将自定义状态转换为客户端状态
impl From<Status> for ClientStatus {
    fn from(val: Status) -> Self {
        match val {
            Status::Unlocked => ClientStatus::Unlocked,
            Status::Locked => ClientStatus::Locked,
        }
    }
}

// 使用 Tauri 框架的异步命令宏定义一个异步函数，该函数处理创建操作
// 创建链接通信的代码初始地方
#[tauri::command(async)]
async fn create(
    // 函数参数：表示操作的当前状态
    status: Status,
    // 函数参数：表示身份凭证的当前状态，使用'static 生命周期引用
    cred: CredentialState<'_>,
    // 函数参数：表示客户端状态的当前状态，使用'static 生命周期引用
    state: ClientState<'_>,
) -> TauriResult<i32> {
    // 从身份凭证状态中读取用户ID和访问令牌，如果为空则返回错误
    let Some((user_id, access_token)) = cred
        .read()
        .as_ref()
        .map(|cred| (cred.user_id, cred.access_token.clone()))
    else {
        return Err(anyhow!("not logon").into());
    };
    // println!("Hello, World!555");
    // println!("{:?}", status);
    // 使用客户端状态的create方法创建一个新的客户端
    state
        .create(
            // 将传入的状态转换为相应的状态类型
            status.into(),
            // 创建客户端时使用的凭证信息
            Credential {
                access_token: &access_token,
                device_uuid: &get_system_info().device.device_uuid,
            },
            // 将用户ID转换为usize类型
            user_id as _,
        )
        // 等待创建完成，如果失败则返回错误信息
        .await
        .context("cannot create client")?;

    println!("Hello, World!3433");
    // 返回成功的结果
    Ok(0)
}

// 定义命令，用于销毁插件
#[tauri::command]
fn destroy(state: ClientState<'_>) -> TauriResult<()> {
    state.destroy()?;

    Ok(())
}

// 定义异步命令，用于获取下一个客户端事件
#[tauri::command(async)]
async fn next_event(client: ClientState<'_>) -> TauriResult<Option<ClientEvent>> {
    println!("next_event");
    // 调用 poll_fn 宏，创建一个异步函数，返回一个 Result<Option<ClientEvent>, TauriError>
    Ok(poll_fn(|cx| {
        // 使用 client.with_mut 调用 Inner 结构体的 poll_recv 方法，尝试接收客户端事件
        let poll_result = client.with_mut(|inner| inner.event_rx.poll_recv(cx));
        println!("Poll Result: {:?}", poll_result);
        if let Ok(poll) = client.with_mut(|inner| inner.event_rx.poll_recv(cx)) {
            poll // 返回 poll 的结果
        } else {
            // 如果接收失败，返回 Pending，表示事件还未就绪
            Poll::Pending
        }
    })
    .await
    .transpose()?)// 对 poll_fn 的结果进行 await 和 transpose 处理，最终返回一个 Result<Option<ClientEvent>, TauriError>
}

// 定义内部结构体包含HeadlessTalk实例和事件接收器
#[derive(Debug)]
struct Inner {
    talk: Arc<HeadlessTalk>,
    event_rx: mpsc::Receiver<anyhow::Result<ClientEvent>>,
}

// 定义客户端结构体，包含读写锁和内部结构体的Option
#[derive(Debug)]
struct Client(RwLock<Option<Inner>>);

// 实现客户端结构体的方法
impl Client {
    const fn new() -> Self {
        Self(RwLock::new(None))
    }

    async fn create(
        &self,
        status: ClientStatus,
        credential: Credential<'_>,
        user_id: i64,
    ) -> anyhow::Result<()> {
        let info = get_system_info();

        let user_dir = info.data_dir.join("userdata").join({
            let mut digest = Sha256::new();

            digest.update("user_");
            digest.update(format!("{user_id}"));

            hex::encode(digest.finalize())
        });

        tokio::fs::create_dir_all(&user_dir)
            .await
            .context("cannot create user directory")?;

        let checkin = checkin(user_id).await?;

        let client = LocoClient::new(
            create_secure_stream((checkin.host.as_str(), checkin.port as u16))
                .await
                .context("failed to create secure stream")?,
        );

        let initializer = TalkInitializer::new(
            client,
            ClientEnv {
                os: TALK_OS,
                net_type: TALK_NET_TYPE,
                app_version: TALK_APP_VERSION,
                mccmnc: TALK_MCCMNC,
                language: info.device.language(),
            },
            user_dir.join("client.db").to_string_lossy(),
        )
        .await
        .context("failed to login")?;

        let (event_tx, event_rx) = mpsc::channel(100);

        let talk = initializer
            .login(credential, status, move |res| {
                let event_tx = event_tx.clone();

                async move {
                    match res {
                        Ok(event) => {
                            if let Err(err) = handle_event(event, event_tx.clone()).await {
                                let _ = event_tx.send(Err(err)).await;
                            }
                        }

                        Err(err) => {
                            let _ = event_tx.send(Err(err.into())).await;
                        }
                    }
                }
            })
            .await
            .context("failed to initialize client")?;

        *self.0.write() = Some(Inner {
            talk: Arc::new(talk),
            event_rx,
        });

        Ok(())
    }

    fn created(&self) -> bool {
        self.0.read().is_some()
    }

    fn with<T>(&self, f: impl FnOnce(&Inner) -> T) -> anyhow::Result<T> {
        Ok(f(self
            .0
            .read()
            .as_ref()
            .ok_or_else(|| anyhow!("client is not created"))?))
    }

    fn with_mut<T>(&self, f: impl FnOnce(&mut Inner) -> T) -> anyhow::Result<T> {
        Ok(f(self
            .0
            .write()
            .as_mut()
            .ok_or_else(|| anyhow!("client is not created"))?))
    }

    fn talk(&self) -> anyhow::Result<Arc<HeadlessTalk>> {
        self.with(|inner| inner.talk.clone())
    }

    fn destroy(&self) -> anyhow::Result<()> {
        self.0
            .write()
            .take()
            .ok_or_else(|| anyhow!("client is not created"))?;

        Ok(())
    }
}
