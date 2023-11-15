import { tauri } from '@tauri-apps/api';

import { ChannelListItem, ClientStatus, KiwiTalkEvent } from '../_types';

// 调用create方法前会调用created方法判断客户端创建的状态
export function created(): Promise<boolean> {
  return tauri.invoke('plugin:client|created');
}

export function create(status: ClientStatus): Promise<number> {
  return tauri.invoke('plugin:client|create', { status });
}

export function destroy(): Promise<void> {
  return tauri.invoke('plugin:client|destroy');
}

export function nextEvent(): Promise<KiwiTalkEvent | null> {
  return tauri.invoke('plugin:client|next_event');
}

export function getChannelList(): Promise<[string, ChannelListItem][]> {
  return tauri.invoke('plugin:client|channel_list');
}

export async function* createMainEventStream(): AsyncGenerator<KiwiTalkEvent> {
  let event: KiwiTalkEvent | null;

  while ((event = await nextEvent())) {
    console.log('await nextEvent()', event);
    yield event;
  }
  /**
   * 在当前代码测试过程中，代码没有执行到这里，说明nextEvent接口一直处于等待中了
   */
  console.log('执行到这里说明createMainEventStream方法内部迭代结束');
}
