'use client';

import { useState } from 'react';

const navigation = [['▦', '概览'], ['◇', '我的任务'], ['⌘', '代码仓库'], ['◉', '服务监控'], ['◫', '知识库']];
const initialTasks = [
  { title: '完成支付 SDK 的错误重试策略', meta: 'xlt-token · 今天', done: false, tag: '深度工作' },
  { title: 'Review #248：认证中间件重构', meta: 'xlt-workbench · 14:00', done: false, tag: '代码评审' },
  { title: '整理本周技术周报', meta: '个人 · 周五', done: false, tag: '写作' },
];
const pullRequests = [
  { id: '#248', title: 'feat: 优化 token 刷新流程', repo: 'xlt-token', status: '等待你审查', tone: 'amber' },
  { id: '#246', title: 'fix: 解决并发登录时的竞态', repo: 'xlt-workbench', status: 'CI 通过', tone: 'green' },
  { id: '#241', title: 'chore: 升级依赖与构建链路', repo: 'xlt-blog', status: '2 条讨论', tone: 'violet' },
];

export default function Home() {
  const [activeNav, setActiveNav] = useState('概览');
  const [tasks, setTasks] = useState(initialTasks);
  const [focus, setFocus] = useState(false);
  const completed = tasks.filter((task) => task.done).length;
  return <main className="workbench-shell">
    <aside className="sidebar">
      <div className="brand" aria-label="Devdeck 工作台"><span className="brand-mark">&lt;/&gt;</span><span>devdeck</span></div>
      <nav aria-label="主导航"><p className="nav-label">工作台</p>{navigation.map(([icon, label]) => <button key={label} className={`nav-item ${activeNav === label ? 'active' : ''}`} onClick={() => setActiveNav(label)}><span>{icon}</span>{label}</button>)}</nav>
      <div className="spaces"><p className="nav-label">空间</p><button className="space-item"><span className="space-dot coral" />XLT 工作区 <small>8</small></button><button className="space-item"><span className="space-dot blue" />个人实验室 <small>3</small></button><button className="new-space">+ 新建空间</button></div>
      <div className="profile"><span className="avatar">W</span><div><strong>Wei Peng</strong><small>在线 · Shanghai</small></div><span>⌄</span></div>
    </aside>
    <section className="workspace">
      <header className="topbar"><div className="crumb"><span>星期二，8月25日</span><i /> <strong>{activeNav}</strong></div><div className="top-actions"><button className="shortcut">⌘ K <span>搜索</span></button><button aria-label="通知" className="icon-button">♧<b /></button><button className="help">?</button></div></header>
      <div className="content">
        <section className="hero"><div><p className="eyebrow">GOOD MORNING, WEI</p><h1>把注意力放在<br /><em>重要的事情</em>上。</h1><p className="hero-copy">今天有 3 个优先事项，两个服务需要留意。</p></div><button className={`focus-card ${focus ? 'running' : ''}`} onClick={() => setFocus(!focus)}><span className="pulse" /><div><small>{focus ? '正在专注' : '专注模式'}</small><strong>{focus ? '24:38' : '开始一个专注时段'}</strong></div><span className="play">{focus ? 'Ⅱ' : '▶'}</span></button></section>
        <section className="metrics" aria-label="今日概况"><article><span className="metric-icon blue-bg">✓</span><div><p>今日任务</p><strong>{completed}<small> / {tasks.length}</small></strong></div><span className="metric-note">{tasks.length - completed} 待完成</span></article><article><span className="metric-icon amber-bg">⌘</span><div><p>提交次数</p><strong>12</strong></div><span className="metric-note up">↑ 20%</span></article><article><span className="metric-icon green-bg">◉</span><div><p>服务状态</p><strong>8<small> / 9</small></strong></div><span className="metric-note">1 个告警</span></article></section>
        <div className="dashboard-grid">
          <section className="panel tasks-panel"><div className="panel-head"><div><p className="eyebrow">FOCUS LIST</p><h2>今天要做什么？</h2></div><button className="text-button">查看全部 →</button></div><div className="task-list">{tasks.map((task, index) => <label className={`task ${task.done ? 'done' : ''}`} key={task.title}><input type="checkbox" checked={task.done} onChange={() => setTasks(tasks.map((item, i) => i === index ? { ...item, done: !item.done } : item))} /><span className="check">✓</span><div><strong>{task.title}</strong><small>{task.meta}</small></div><em>{task.tag}</em></label>)}</div><button className="add-task" onClick={() => setTasks([...tasks, { title: '记录一个新的想法', meta: '刚刚创建', done: false, tag: '待分类' }])}>+ 添加任务</button></section>
          <section className="panel activity-panel"><div className="panel-head"><div><p className="eyebrow">PULSE</p><h2>开发动态</h2></div><button className="dots">•••</button></div><div className="commit"><span className="commit-node green" /><div><strong>你推送了 3 个提交</strong><p><code>feat/auth</code> → main</p><small>12 分钟前 · xlt-token</small></div></div><div className="commit"><span className="commit-node violet" /><div><strong>Lin Chen 请求你的审查</strong><p><code>#248</code> 优化 token 刷新流程</p><small>34 分钟前 · xlt-workbench</small></div></div><div className="commit"><span className="commit-node blue" /><div><strong>部署已完成</strong><p>production · <code>v2.14.0</code></p><small>1 小时前 · xlt-blog</small></div></div><button className="activity-link">查看全部动态 →</button></section>
        </div>
        <section className="panel pr-panel"><div className="panel-head"><div><p className="eyebrow">CODE REVIEW</p><h2>需要你关注的 Pull Requests</h2></div><button className="text-button">打开 GitHub ↗</button></div><div className="pr-list">{pullRequests.map(pr => <article className="pr-row" key={pr.id}><span className={`pr-state ${pr.tone}`}>⎇</span><div><strong>{pr.id} · {pr.title}</strong><small>{pr.repo}</small></div><span className={`status ${pr.tone}`}>{pr.status}</span><button className="row-arrow">→</button></article>)}</div></section>
      </div>
    </section>
  </main>;
}
