import {
  Accessor,
  ParentProps,
  createContext,
  createResource,
  createSignal,
  useContext,
} from 'solid-js';


import { created, create, createMainEventStream } from '@/api/client/client';
import { KiwiTalkEvent, LogoutReason } from '@/api';
import { createSelfChannelEventStream } from '../_utils';

const ReadyContext = createContext<Accessor<boolean>>(() => false);
export const useReady = () => useContext(ReadyContext);

// 定义 ReadyProvider的props类型
export type ReadyProviderProps = ParentProps<{
  onReady?: () => void; // 回调函数，当应用程序准备就绪时调用
  onLogout?: (reason: LogoutReason) => void; // 回调函数，当用户注销时调用
  onEvent?: (event: KiwiTalkEvent) => void; // 回调函数，处理KiwTalk事件
}>;

// 定义ReadyProvider组件
export const ReadyProvider = (props: ReadyProviderProps) => {
  // 创建一个Signal来表示准备状态，并提供修改状态的函数
  const [isReady, setIsReady] = createSignal(false);

  // 用于标记是否已完成处理所有事件
  let finished = false;

  /**
   * @description 创建资源，处理主要事件流
   */
  createResource(async () => {
    // 如果客户端未创建成功，则发送创建请求
    let isCreated = created();
    console.log();
    if (!await created()) {
      console.log('当前创建状态是否：Unlocked');
      await create('Unlocked');
    }

    setIsReady(true);
    props.onReady?.();

    // 创建主要事件流
    const stream = createMainEventStream();
    console.log('执行到此处说明setIsReady(true)方法调用了');
    try {
      console.log('调用createResource方法，成功执行到其内部try模块');
      console.log('stream', stream);
      for await (const event of stream) {
        console.log('event', event);

        // 如果事件类型是'Kickout'（退出），则触发onLogout回调，并标记为已完成
        if (event.type === 'Kickout') {
          props.onLogout?.({ type: 'Kickout', reasonId: event.content.reason });
          finished = true;
          return;
        }

        // 否则触发onEvent事件回调
        props.onEvent?.(event);
      }

      console.log('finished: ', finished);
      // 如果已完成处理，直接返回
      if (finished) return;

      // 如果主要事件流结束，触发 onLogout 回调，标记为 'Disconnected'（客户端连接断开类型）
      props.onLogout?.({ type: 'Disconnected' });
    } catch (err) {
      // 如果捕获到错误，触发 onLogout 回调，标记为 'Error'
      props.onLogout?.({ type: 'Error', err });
      console.log("props.onLogout?.({ type: 'Error', err });");
    } finally {
      console.log('这句话被打印说明try语句内等待异步结果不影响执行finally');
      finished = true;
    }
  });

  /**
   * @description 创建资源，处理自身判断事件流
   * 该方法会在页面刷新的时候被调用
   */
  createResource(async () => {
    const stream = createSelfChannelEventStream();
    console.log('createResource被调用', stream);
    try {
      for await (const event of stream) {
        props.onEvent?.({
          type: 'Channel',
          content: {
            id: event.id,
            event,
          },
        });
      }
    } catch (err) {
      props.onLogout?.({ type: 'Error', err });
    }
  });

  // 渲染ReadyContext.Provider, 提供准备状态给子组件使用
  return (
    <ReadyContext.Provider value={isReady}>
      {props.children}
    </ReadyContext.Provider>
  );
};
