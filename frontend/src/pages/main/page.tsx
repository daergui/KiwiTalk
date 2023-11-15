import { createSignal } from 'solid-js';
import { Outlet, useLocation, useNavigate } from '@solidjs/router';

import { KiwiTalkEvent, LogoutReason } from '@/api';
import { destroy } from '@/api/client/client';

import { Sidebar } from './_components/sidebar';
import { ReadyProvider, EventContext, useSidebar } from './_hooks';

import * as styles from './page.css';

export const MainPage = () => {
  const navigate = useNavigate();
  const location = useLocation();

  const [isReady, setIsReady] = createSignal(false);
  const [listeners, setListeners] = createSignal<((event: KiwiTalkEvent) => void)[]>([]);

  const sidebar = useSidebar(isReady);

  const activeTab = () => location.pathname.match(/main\/([^/]+)/)?.[1] ?? 'chat';
  const setActiveTab = (tab: string) => {
    navigate(`${tab}`, { replace: true });
  };

  const onLogout = async (reason: LogoutReason) => {
    try {
      navigate('/login', { resolve: false, replace: true });
      console.log('logout', reason);
    } finally {
      await destroy();
    }
  };
  const onEvent = (event: KiwiTalkEvent) => {
    listeners().forEach((listener) => listener(event));
  };

  return (
    <EventContext.Provider value={{
      addEvent: (listener) => {
        setListeners([...listeners(), listener]);
      },
      removeEvent: (listener) => {
        const newList = listeners().filter((l) => l !== listener);

        setListeners(newList);
      },
    }}>
      <ReadyProvider onLogout={onLogout} onEvent={onEvent} onReady={() => setIsReady(true)}>
        <main class={styles.container}>
          <div class={styles.sidebarWrapper}>
            000
            <Sidebar
              collapsed={false}
              activePath={activeTab()}
              setActivePath={setActiveTab}

              chatBadges={sidebar.badges()?.chat}
              openChatBadges={sidebar.badges()?.open}
              notificationActive={sidebar.notificationActive()}
              onNotificationActive={sidebar.setNotificationActive}
            />
          </div>
          555
          <Outlet />
          666
        </main>
      </ReadyProvider>
    </EventContext.Provider>
  );
};
