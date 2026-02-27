import { createRouter, createWebHistory } from 'vue-router'

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: '/',
      name: 'dashboard',
      component: () => import('../views/DashboardView.vue'),
      meta: { title: '仪表盘' },
    },
    {
      path: '/playlist',
      name: 'playlist',
      component: () => import('../views/PlaylistView.vue'),
      meta: { title: '播放列表' },
    },
    {
      path: '/mixer',
      name: 'mixer',
      component: () => import('../views/MixerView.vue'),
      meta: { title: '混音器' },
    },
    {
      path: '/playing',
      name: 'playing',
      component: () => import('../views/PlayingView.vue'),
      meta: { title: '正在播放' },
    },
    {
      path: '/status',
      name: 'status',
      component: () => import('../views/StatusView.vue'),
      meta: { title: '状态' },
    },
    {
      path: '/settings',
      name: 'settings',
      component: () => import('../views/SettingsView.vue'),
      meta: { title: '设置' },
    },
  ],
})

export default router
