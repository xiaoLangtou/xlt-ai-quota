<script lang="ts" setup>
import { computed, ref, watch } from "vue";
import {
    ArrowRight,
    CalendarDays,
    Check,
    ChevronDown,
    Copy,
    FileText,
    FolderOpen,
    GitBranch,
} from "lucide-vue-next";
import { open } from "@tauri-apps/plugin-dialog";
import { dailyReportService } from "@/services/daily-report-service";
import { AI_PROVIDERS } from "@/config/ai-providers";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { DatePicker } from "@/components/ui/date-picker";
import { Input } from "@/components/ui/input";
import { Popover } from "@/components/ui/popover";
import { Textarea } from "@/components/ui/textarea";
import { pushToast } from "@/composables/useToast";
import type { AiProvider, GitProjectCommits, ReportKind } from "@/types/daily-report";

type DatePreset = "today" | "yesterday" | "week" | "lastweek" | "custom";
type WorkspaceTab = "report" | "commits";
type ReportPopover = "range" | "projects" | "authors";

const DATE_PRESETS: ReadonlyArray<readonly [ DatePreset, string ]> = [
    [ "today", "今天" ],
    [ "yesterday", "昨天" ],
    [ "week", "本周" ],
    [ "lastweek", "上周" ],
    [ "custom", "自定义" ],
];

function localDate(value: Date): string {
    const year = value.getFullYear();
    const month = String(value.getMonth() + 1).padStart(2, "0");
    const day = String(value.getDate()).padStart(2, "0");
    return `${ year }-${ month }-${ day }`;
}

function dateOffset(date: string, offset: number): string {
    const value = new Date(`${ date }T12:00:00`);
    value.setDate(value.getDate() + offset);
    return localDate(value);
}

function mondayOf(date: string): string {
    const value = new Date(`${ date }T12:00:00`);
    const day = value.getDay() || 7;
    value.setDate(value.getDate() - day + 1);
    return localDate(value);
}

const today = localDate(new Date());
const preferences = ref(dailyReportService.loadPreferences());
const selectedPaths = ref(new Set(preferences.value.projects.map((project) => project.path)));
const reportKind = ref<ReportKind>("daily");
const datePreset = ref<DatePreset>("today");
const customStartDate = ref(today);
const customEndDate = ref(today);
const projectPath = ref("");
const authorDraft = ref("");
const aiProvider = ref<AiProvider>(preferences.value.ai?.provider ?? "minimax");
const aiModel = ref(preferences.value.ai?.model ?? AI_PROVIDERS.minimax.models[0]);
const aiApiKey = ref(preferences.value.aiApiKeys?.[preferences.value.ai?.provider ?? "minimax"] ?? "");
const activePopover = ref<ReportPopover | null>(null);
const workspaceTab = ref<WorkspaceTab>("report");
const commits = ref<GitProjectCommits[]>([]);
const generatedReport = ref("");
const hasLoaded = ref(false);
const readingCommits = ref(false);
const generatingReport = ref(false);
const addingProject = ref(false);
const importingGitReport = ref(false);
const copied = ref(false);
const message = ref("");
const error = ref("");

const selectedProjects = computed(() => preferences.value.projects.filter((project) => selectedPaths.value.has(project.path)));
const dateRange = computed(() => {
    if ( datePreset.value === "today" ) return { startDate: today, endDate: today };
    if ( datePreset.value === "yesterday" ) {
        const yesterday = dateOffset(today, -1);
        return { startDate: yesterday, endDate: yesterday };
    }
    if ( datePreset.value === "week" ) return { startDate: mondayOf(today), endDate: today };
    if ( datePreset.value === "lastweek" ) {
        const endDate = dateOffset(mondayOf(today), -1);
        return { startDate: dateOffset(endDate, -6), endDate };
    }
    return { startDate: customStartDate.value, endDate: customEndDate.value };
});
const rangeLabel = computed(() => dateRange.value.startDate === dateRange.value.endDate
    ? dateRange.value.startDate
    : `${ dateRange.value.startDate } ～ ${ dateRange.value.endDate }`);
const datePresetLabel = computed(() => DATE_PRESETS.find(([ value ]) => value === datePreset.value)?.[1] ?? "自定义");
const totalCommitCount = computed(() => commits.value.reduce((total, item) => total + item.commits.length, 0));
const flatCommits = computed(() => commits.value.flatMap((group) => group.commits.map((commit) => ( {
    ...commit,
    project: group.project.name,
    branch: group.project.branch,
} ))));
const reportLabel = computed(() => reportKind.value === "daily" ? "日报" : "周报");

watch(message, (value) => {
    if ( value ) pushToast(value, "success");
});
watch(error, (value) => {
    if ( value ) pushToast(value, "error");
});
watch([ reportKind, datePreset, customStartDate, customEndDate ], () => clearResult());
watch(generatedReport, () => {
    copied.value = false;
});

function clearResult(): void {
    commits.value = [];
    generatedReport.value = "";
    hasLoaded.value = false;
    workspaceTab.value = "report";
}

function resetFeedback(): void {
    message.value = "";
    error.value = "";
}

function setPopoverOpen(name: ReportPopover, open: boolean): void {
    if ( open ) activePopover.value = name;
    else if ( activePopover.value === name ) activePopover.value = null;
}

function savePreferences(): void {
    dailyReportService.savePreferences(preferences.value);
}

function setReportKind(value: ReportKind): void {
    reportKind.value = value;
    datePreset.value = value === "daily" ? "today" : "week";
}

function setDatePreset(value: DatePreset): void {
    datePreset.value = value;
}

function toggleProject(path: string): void {
    const next = new Set(selectedPaths.value);
    if ( next.has(path) ) next.delete(path);
    else next.add(path);
    selectedPaths.value = next;
    clearResult();
}

function selectAllProjects(): void {
    selectedPaths.value = new Set(preferences.value.projects.map((project) => project.path));
    clearResult();
}

function selectNoProjects(): void {
    selectedPaths.value = new Set();
    clearResult();
}

function addAuthor(): void {
    const author = authorDraft.value.trim();
    if ( !author || preferences.value.authors.includes(author) ) return;
    preferences.value.authors.push(author);
    authorDraft.value = "";
    savePreferences();
    clearResult();
}

function removeAuthor(author: string): void {
    preferences.value.authors = preferences.value.authors.filter((item) => item !== author);
    savePreferences();
    clearResult();
}

async function addGitProject(): Promise<void> {
    resetFeedback();
    const path = projectPath.value.trim();
    if ( !path ) {
        error.value = "请输入 Git 项目的绝对路径。";
        return;
    }
    addingProject.value = true;
    try {
        const project = await dailyReportService.validateProject(path);
        if ( preferences.value.projects.some((item) => item.path === project.path) ) {
            error.value = "该 Git 项目已经添加。";
            return;
        }
        preferences.value.projects.push(project);
        selectedPaths.value = new Set([ ...selectedPaths.value, project.path ]);
        savePreferences();
        projectPath.value = "";
        message.value = `已添加 ${ project.name }。`;
    } catch ( reason ) {
        error.value = reason instanceof Error ? reason.message : String(reason);
    } finally {
        addingProject.value = false;
    }
}

async function chooseGitProject(): Promise<void> {
    resetFeedback();
    try {
        const selected = await open({ directory: true, multiple: false, title: "选择 Git 项目文件夹" });
        if ( typeof selected === "string" ) projectPath.value = selected;
    } catch ( reason ) {
        error.value = reason instanceof Error ? reason.message : String(reason);
    }
}

function removeGitProject(path: string): void {
    preferences.value.projects = preferences.value.projects.filter((item) => item.path !== path);
    const next = new Set(selectedPaths.value);
    next.delete(path);
    selectedPaths.value = next;
    savePreferences();
    clearResult();
}

async function importGitReportConfig(): Promise<void> {
    resetFeedback();
    importingGitReport.value = true;
    try {
        const imported = await dailyReportService.importGitReportConfig();
        const existing = new Set(preferences.value.projects.map((project) => project.path));
        const additions = imported.projects.filter((project) => !existing.has(project.path));
        preferences.value.projects.push(...additions);
        selectedPaths.value = new Set([ ...selectedPaths.value, ...additions.map((project) => project.path) ]);
        preferences.value.authors = [ ...new Set([ ...preferences.value.authors, ...imported.authors ]) ];
        savePreferences();
        message.value = `已导入 ${ additions.length } 个 GitReport 项目。`;
    } catch ( reason ) {
        error.value = reason instanceof Error ? reason.message : String(reason);
    } finally {
        importingGitReport.value = false;
    }
}

async function generateReport(): Promise<void> {
    resetFeedback();
    if ( !aiApiKey.value.trim() ) {
        error.value = "请先填写 AI Provider 的 API Key。";
        return;
    }
    if ( !selectedProjects.value.length ) {
        error.value = "请至少选择一个 Git 项目。";
        return;
    }
    generatingReport.value = true;
    workspaceTab.value = "report";
    try {
        savePreferences();
        commits.value = await dailyReportService.collectCommits({
            projects: selectedProjects.value,
            authors: preferences.value.authors,
            ...dateRange.value,
        });
        hasLoaded.value = true;
        generatedReport.value = await dailyReportService.generateReport({
            provider: aiProvider.value,
            model: aiModel.value,
            apiKey: aiApiKey.value,
            reportType: reportKind.value,
            ...dateRange.value,
            projects: commits.value,
        });
        message.value = `${ reportLabel.value }已生成。`;
    } catch ( reason ) {
        error.value = reason instanceof Error ? reason.message : String(reason);
    } finally {
        generatingReport.value = false;
    }
}

async function readGitCommits(): Promise<void> {
    resetFeedback();
    if ( !selectedProjects.value.length ) {
        error.value = "请至少选择一个 Git 项目。";
        return;
    }
    readingCommits.value = true;
    try {
        savePreferences();
        commits.value = await dailyReportService.collectCommits({
            projects: selectedProjects.value,
            authors: preferences.value.authors,
            ...dateRange.value,
        });
        generatedReport.value = "";
        hasLoaded.value = true;
        workspaceTab.value = "commits";
        message.value = `已读取 ${ totalCommitCount.value } 条 Git 提交。`;
    } catch ( reason ) {
        error.value = reason instanceof Error ? reason.message : String(reason);
    } finally {
        readingCommits.value = false;
    }
}

async function copyReport(): Promise<void> {
    resetFeedback();
    if ( !generatedReport.value.trim() ) {
        error.value = "请先生成或填写报告正文。";
        return;
    }
    try {
        await navigator.clipboard.writeText(generatedReport.value.trim());
        copied.value = true;
        message.value = "报告内容已复制。";
    } catch ( reason ) {
        error.value = reason instanceof Error ? reason.message : "无法写入系统剪贴板。";
    }
}
</script>

<template>
    <div class="git-report-page">
        <section aria-label="Git 报告配置" class="report-toolbar">
            <div class="toolbar-left">
                <div aria-label="报告类型" class="toolbar-group type-segment" role="group">
                    <button :class="{ active: reportKind === 'daily' }" type="button" @click="setReportKind('daily')">
                        日报
                    </button>
                    <button :class="{ active: reportKind === 'weekly' }" type="button" @click="setReportKind('weekly')">
                        周报
                    </button>
                </div>

                <div class="toolbar-group">
                    <Popover :open="activePopover === 'range'" align="start" content-class="report-popover"
                             @update:open="setPopoverOpen('range', $event)">
                        <template #trigger>
                            <CalendarDays :size="15"/>
                            <span>时间范围</span>
                            <code>{{ datePresetLabel }}</code>
                            <ChevronDown :size="13"/>
                        </template>
                        <p class="popover-title">选择时间范围</p>
                        <div class="preset-chips">
                            <Button v-for="preset in DATE_PRESETS" :key="preset[0]" :class="{ active: datePreset === preset[0] }" size="sm"
                                    type="button" variant="outline"
                                    @click="setDatePreset(preset[0])">{{ preset[1] }}
                            </Button>
                        </div>
                        <div v-if="datePreset === 'custom'" class="custom-dates">
                            <label><span>开始</span>
                                <DatePicker v-model="customStartDate"/>
                            </label>
                            <label><span>结束</span>
                                <DatePicker v-model="customEndDate"/>
                            </label>
                        </div>
                        <div class="date-readout">{{ rangeLabel }}</div>
                    </Popover>
                </div>

                <div class="toolbar-group">
                    <Popover :open="activePopover === 'projects'" align="start"
                             content-class="report-popover projects-popover"
                             @update:open="setPopoverOpen('projects', $event)">
                        <template #trigger>
                            <FolderOpen :size="15"/>
                            <span>项目</span>
                            <code>{{ selectedProjects.length }}/{{ preferences.projects.length }}</code>
                            <ChevronDown :size="13"/>
                        </template>
                        <div class="popover-head">
                            <p class="popover-title">选择仓库</p>
                            <div class="popover-links">
                                <Button size="sm" type="button" variant="ghost" @click="selectAllProjects">全选</Button>
                                <Button size="sm" type="button" variant="ghost" @click="selectNoProjects">清空</Button>
                                <Button :disabled="importingGitReport" size="sm" type="button" variant="ghost"
                                        @click="importGitReportConfig">{{ importingGitReport ? "导入中" : "导入" }}
                                </Button>
                            </div>
                        </div>
                        <div v-if="preferences.projects.length" class="project-list">
                            <article v-for="project in preferences.projects" :key="project.path" :class="{ selected: selectedPaths.has(project.path) }"
                                     class="project-row"
                                     @click="toggleProject(project.path)">
                                <Checkbox :model-value="selectedPaths.has(project.path)" class="project-check"
                                          tabindex="-1"/>
                                <div class="project-meta">
                                    <div><strong>{{ project.name }}</strong><span>{{ project.branch }}</span></div>
                                    <small>{{ project.path }}</small>
                                </div>
                                <Button :aria-label="`移除 ${project.name}`" class="project-remove" size="icon" type="button"
                                        variant="ghost"
                                        @click.stop="removeGitProject(project.path)">×
                                </Button>
                            </article>
                        </div>
                        <p v-else class="popover-empty">还没有添加项目</p>
                        <div class="project-add">
                            <Input v-model="projectPath" placeholder="输入 Git 项目绝对路径"
                                   @keyup.enter="addGitProject"/>
                            <Button aria-label="选择 Git 项目文件夹" size="icon" title="选择文件夹" type="button"
                                    variant="outline" @click="chooseGitProject">
                                <FolderOpen :size="14"/>
                            </Button>
                            <Button :disabled="addingProject" size="sm" type="button" variant="outline"
                                    @click="addGitProject">{{ addingProject ? "…" : "添加" }}
                            </Button>
                        </div>
                    </Popover>
                </div>

                <div class="toolbar-group">
                    <Popover :open="activePopover === 'authors'" align="start" content-class="report-popover"
                             @update:open="setPopoverOpen('authors', $event)">
                        <template #trigger>
                            <span aria-hidden="true" class="person-icon"/>
                            <span>作者</span>
                            <code>{{ preferences.authors.length }} 人</code>
                            <ChevronDown :size="13"/>
                        </template>
                        <p class="popover-title">按作者过滤</p>
                        <div class="author-field">
                            <span v-for="author in preferences.authors" :key="author" class="author-chip">{{ author }}<button
                                :aria-label="`移除作者 ${author}`" type="button"
                                @click="removeAuthor(author)">×</button></span>
                            <input v-model="authorDraft" placeholder="输入后回车" type="text"
                                   @keydown.enter.prevent="addAuthor"/>
                        </div>
                    </Popover>
                </div>
            </div>

            <div class="toolbar-actions">
                <Button :disabled="generatingReport || readingCommits" class="read-action" size="sm" type="button"
                        variant="ghost" @click="readGitCommits">
                    {{ readingCommits ? "正在读取…" : "仅读取 Git 记录" }}
                </Button>
                <Button :disabled="generatingReport || readingCommits" class="generate-action" type="button"
                        @click="generateReport">
                    <ArrowRight :size="15"/>
                    {{ generatingReport ? "正在生成…" : `生成${ reportLabel }` }}
                </Button>
            </div>
        </section>

        <main class="report-canvas">
            <header class="canvas-tabs">
                <div aria-label="报告视图" role="tablist">
                    <button :aria-selected="workspaceTab === 'report'" :class="{ active: workspaceTab === 'report' }" role="tab"
                            type="button" @click="workspaceTab = 'report'">报告预览
                    </button>
                    <button :aria-selected="workspaceTab === 'commits'" :class="{ active: workspaceTab === 'commits' }" role="tab"
                            type="button" @click="workspaceTab = 'commits'">Git
                        记录<span v-if="hasLoaded">{{ totalCommitCount }}</span></button>
                </div>
                <span class="badge">{{
                        hasLoaded ? `${ totalCommitCount } 条提交` : `${ selectedProjects.length } 个仓库`
                    }}</span>
            </header>

            <section v-if="workspaceTab === 'report'" class="canvas-body">
                <div v-if="generatingReport" class="loading-state"><span>正在整理提交记录<i/></span></div>
                <div v-else-if="!hasLoaded" class="empty-state">
                    <FileText :size="54" stroke-width="1.15"/>
                    <h2>在上方选好条件，点击「生成{{ reportLabel }}」</h2>
                    <p>AI 会读取所选仓库在这段时间内的提交，按项目整理成可以直接复制粘贴的工作报告。</p>
                </div>
                <article v-else class="report-document">
                    <header class="document-head">
                        <div><span>{{ reportLabel }}</span><strong>{{
                                rangeLabel
                            }}</strong><span>共 {{ selectedProjects.length }} 个仓库</span></div>
                        <button :class="{ copied }" :disabled="!generatedReport.trim()" type="button"
                                @click="copyReport">
                            <Check v-if="copied" :size="13"/>
                            <Copy v-else :size="13"/>
                            {{ copied ? "已复制" : "复制" }}
                        </button>
                    </header>
                    <Textarea v-model="generatedReport" :placeholder="`点击上方“生成${reportLabel}”后，内容会显示在这里。`"
                              class="report-editor"/>
                    <footer>由 {{ AI_PROVIDERS[aiProvider].label }} · {{ aiModel }} 生成 · {{ selectedProjects.length }}
                        个仓库 · {{ totalCommitCount }} 条提交
                    </footer>
                </article>
            </section>

            <section v-else class="canvas-body">
                <div v-if="readingCommits" class="loading-state"><span>正在读取 Git 记录<i/></span></div>
                <div v-else-if="!hasLoaded" class="empty-state">
                    <GitBranch :size="52" stroke-width="1.15"/>
                    <h2>尚未读取 Git 记录</h2>
                    <p>选择项目与时间范围后，点击上方「仅读取 Git 记录」。</p>
                </div>
                <div v-else-if="flatCommits.length" class="commit-list">
                    <article v-for="commit in flatCommits" :key="`${commit.project}-${commit.hash}`" class="commit-row">
                        <code>{{ commit.hash.slice(0, 7) }}</code>
                        <div><strong>{{ commit.message }}</strong><span>{{ commit.project }} · {{
                                commit.branch
                            }}</span></div>
                        <time>{{ commit.date }}</time>
                    </article>
                </div>
                <div v-else class="empty-state compact">
                    <GitBranch :size="44" stroke-width="1.15"/>
                    <h2>没有匹配的提交</h2>
                    <p>{{ rangeLabel }} 内未找到符合当前项目与作者条件的 Git 记录。</p>
                </div>
            </section>
        </main>
    </div>
</template>

<style scoped>
.git-report-page {
    --report-panel: var(--surface);
    --report-paper: var(--surface);
    --report-line: color-mix(in srgb, var(--text) 13%, transparent);
    --report-line-strong: color-mix(in srgb, var(--text) 21%, transparent);
    --report-ink: var(--text);
    --report-soft: var(--text-muted);
    --report-muted: var(--text-subtle);
    display: flex;
    min-width: 0;
    min-height: calc(100vh - 146px);
    flex-direction: column;
    gap: 16px;
    color: var(--report-ink);
}
.report-toolbar {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    gap: 12px;
    padding: 6px 8px;
    border: 1px solid var(--report-line);
    border-radius: 12px;
    background: var(--report-panel);
    box-shadow: var(--glass-shadow);
    backdrop-filter: blur(20px) saturate(150%);
}
.toolbar-left,
.toolbar-actions,
.toolbar-group,
.type-segment,
.canvas-tabs,
.canvas-tabs > div,
.document-head,
.document-head > div {
    display: flex;
    align-items: center;
}
.toolbar-left {
    min-width: 0;
    flex-wrap: nowrap;
}
.toolbar-group {
    position: relative;
}
.toolbar-group + .toolbar-group {
    margin-left: 4px;
    padding-left: 4px;
    border-left: 1px solid var(--report-line);
}
.type-segment {
    gap: 2px;
    padding: 3px;
    border: 1px solid var(--border);
    border-radius: 7px;
    background: var(--segment-bg);
}
.type-segment button,
.read-action,
.generate-action,
.canvas-tabs button,
.document-head button {
    border: 0;
    font: inherit;
    cursor: pointer;
}
.type-segment button {
    min-width: 52px;
    padding: 5px 11px;
    border-radius: 5px;
    background: transparent;
    color: var(--text-muted);
    font-size: 12px;
    font-weight: 600;
}
.type-segment button.active {
    background: var(--surface);
    color: var(--accent);
    box-shadow: var(--shadow-sm);
}
.person-icon {
    position: relative;
    width: 15px;
    height: 15px;
    flex: none;
}
.person-icon::before,
.person-icon::after {
    position: absolute;
    left: 50%;
    border: 1.5px solid currentColor;
    content: "";
    transform: translateX(-50%);
}
.person-icon::before {
    top: 0;
    width: 5px;
    height: 5px;
    border-radius: 50%;
}
.person-icon::after {
    bottom: 0;
    width: 11px;
    height: 6px;
    border-bottom: 0;
    border-radius: 8px 8px 0 0;
}
.toolbar-actions {
    flex: none;
    gap: 9px;
}
.read-action {
    padding: 8px 6px;
    background: transparent;
    color: var(--report-muted);
    font-size: 12px;
    white-space: nowrap;
}
.read-action:hover {
    color: var(--report-ink);
}
.generate-action {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 7px;
    padding: 9px 15px;
    border-radius: 7px;
    background: var(--accent);
    color: var(--accent-contrast);
    font-size: 13px;
    font-weight: 700;
    line-height: 1;
    white-space: nowrap;
}
.generate-action svg {
    flex: none;
}
.generate-action:hover {
    background: var(--accent-hover);
}
.read-action:disabled,
.generate-action:disabled {
    cursor: not-allowed;
    opacity: .45;
}
.report-canvas {
    display: flex;
    min-height: 560px;
    flex: 1;
    flex-direction: column;
    overflow: hidden;
    border: 1px solid var(--report-line);
    border-radius: 12px;
    background: var(--report-paper);
    box-shadow: var(--glass-shadow);
    backdrop-filter: blur(20px) saturate(150%);
}
.canvas-tabs {
    min-height: 51px;
    justify-content: space-between;
    padding: 0 16px;
    border-bottom: 1px solid var(--report-line);
}
.canvas-tabs > div {
    align-self: center;
    gap: 2px;
    padding: 3px;
    border: 1px solid var(--report-line);
    border-radius: 7px;
    background: var(--segment-bg);
}
.canvas-tabs button {
    min-width: 72px;
    padding: 5px 11px;
    border: 0;
    border-radius: 5px;
    background: transparent;
    color: var(--report-muted);
    font: inherit;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
}
.canvas-tabs button.active {
    background: var(--surface);
    box-shadow: var(--shadow-sm);
    color: var(--accent);
}
.canvas-tabs button span {
    display: inline-grid;
    min-width: 16px;
    height: 16px;
    place-items: center;
    margin-left: 6px;
    border-radius: 999px;
    background: var(--accent-weak);
    color: var(--accent);
    font-family: var(--font-mono);
    font-size: 9px;
    font-weight: 650;
}
.canvas-tabs > .badge {
    margin-left: auto;
    padding: 2px 8px;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--surface-2);
    color: var(--text-muted);
    font-size: 11px;
    font-weight: 650;
}
.canvas-body {
    display: flex;
    min-height: 0;
    flex: 1;
    flex-direction: column;
    overflow: auto;
}
.empty-state,
.loading-state {
    display: flex;
    min-height: 440px;
    flex: 1;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 40px;
    text-align: center;
}
.empty-state {
    color: var(--report-line-strong);
}
.empty-state h2 {
    margin: 17px 0 7px;
    color: var(--report-ink);
    font-size: 15px;
    font-weight: 700;
}
.empty-state p {
    max-width: 390px;
    margin: 0;
    color: var(--report-muted);
    font-size: 12px;
    line-height: 1.65;
}
.empty-state.compact {
    min-height: 340px;
}
.loading-state span {
    display: inline-flex;
    align-items: center;
    color: var(--report-soft);
    font-family: var(--font-mono);
    font-size: 12px;
}
.loading-state i {
    width: 8px;
    height: 15px;
    margin-left: 3px;
    background: var(--report-ink);
    animation: cursor-blink 1s step-end infinite;
}

@keyframes cursor-blink {
    50% {
        opacity: 0;
    }
}
.report-document {
    display: flex;
    min-height: 100%;
    flex-direction: column;
    padding: 0 24px 26px;
}
.document-head {
    justify-content: space-between;
    gap: 16px;
    margin: 0 -24px 14px;
    padding: 14px 20px;
    border-bottom: 1px solid var(--report-line);
    background: var(--surface-2);
}
.document-head > div {
    gap: 9px;
    color: var(--report-muted);
    font-size: 11.5px;
}
.document-head strong {
    color: var(--report-ink);
    font-size: 13.5px;
    font-weight: 700;
}
.document-head button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 6px 11px;
    border: 1px solid var(--report-line-strong);
    border-radius: 6px;
    background: var(--surface);
    color: var(--report-soft);
    font-size: 11.5px;
    font-weight: 600;
}
.document-head button:hover:not(:disabled),
.document-head button.copied {
    border-color: var(--accent);
    color: var(--accent);
}
.document-head button:disabled {
    cursor: not-allowed;
    opacity: .4;
}
.report-editor {
    width: 100%;
    min-height: 390px;
    flex: 1;
    margin-top: 15px;
    padding: 0;
    border: 0;
    border-radius: 0;
    outline: 0;
    background: transparent;
    box-shadow: none;
    color: var(--report-ink);
    font-family: var(--font-sans);
    font-size: 14px;
    line-height: 1.85;
    resize: vertical;
}
.report-editor:focus {
    box-shadow: none;
}
.report-document footer {
    padding-top: 13px;
    border-top: 1px solid var(--report-line);
    color: var(--report-muted);
    font-family: var(--font-mono);
    font-size: 10px;
}
.commit-list {
    padding: 6px 22px 22px;
}
.commit-row {
    display: grid;
    grid-template-columns: 72px minmax(0, 1fr) auto;
    gap: 14px;
    align-items: center;
    padding: 11px 4px;
    border-bottom: 1px solid var(--report-line);
}
.commit-row > code {
    padding: 2px 7px;
    border-radius: 5px;
    background: var(--accent-weak);
    color: var(--accent);
    font-family: var(--font-mono);
    font-size: 11px;
    text-align: center;
}
.commit-row > div {
    display: grid;
    min-width: 0;
    gap: 2px;
}
.commit-row strong {
    overflow: hidden;
    font-size: 12.5px;
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
}
.commit-row span,
.commit-row time {
    color: var(--report-muted);
    font-family: var(--font-mono);
    font-size: 10px;
}
.commit-row time {
    white-space: nowrap;
}

@media (max-width: 760px) {
.report-toolbar {
        grid-template-columns: 1fr;
        align-items: stretch;
    }
.toolbar-left {
        align-items: stretch;
        flex-direction: column;
    }
.toolbar-group + .toolbar-group {
        margin: 0;
        padding: 3px 0 0;
        border-top: 1px solid var(--report-line);
        border-left: 0;
    }
.type-segment {
        align-self: flex-start;
    }
.toolbar-actions {
        align-items: stretch;
        flex-direction: column-reverse;
    }
.generate-action {
        justify-content: center;
    }
.canvas-tabs {
        align-items: flex-start;
        flex-direction: column;
        gap: 8px;
        padding: 0 15px 10px;
    }
.canvas-tabs > div {
        min-height: 48px;
    }
.document-head {
        align-items: flex-start;
        flex-direction: column;
    }
.commit-row {
        grid-template-columns: 64px minmax(0, 1fr);
    }
.commit-row time {
        grid-column: 2;
    }
}

@media (prefers-reduced-motion: reduce) {
.loading-state i {
        animation: none;
    }
}
</style>

<style>
.report-popover {
    width: 280px;
    padding: 16px;
}
.report-popover.projects-popover {
    width: min(390px, calc(100vw - 28px));
}
.report-popover .popover-title {
    margin: 0 0 11px;
    color: var(--text-subtle);
    font-size: 12px;
}
.report-popover .popover-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    margin-bottom: 9px;
}
.report-popover .popover-head .popover-title {
    margin: 0;
}
.report-popover .popover-links {
    display: flex;
    gap: 2px;
}
.report-popover .popover-links .cn-button {
    height: 26px;
    padding-inline: 7px;
    color: var(--text-muted);
    font-size: 11px;
}
.report-popover .preset-chips {
    display: flex;
    flex-wrap: wrap;
    gap: 7px;
}
.report-popover .cn-button-outline,
.report-popover .cn-input,
.report-popover .cn-select-trigger,
.report-popover .cn-date-picker-trigger {
    border-color: color-mix(in srgb, var(--text) 16%, transparent);
    background: var(--surface-2);
}
.report-popover .cn-button-outline:hover,
.report-popover .cn-input:focus,
.report-popover .cn-select-trigger:focus-visible,
.report-popover .cn-date-picker-trigger:hover {
    border-color: var(--accent);
}
.report-popover .preset-chips .cn-button {
    border-radius: 999px;
}
.report-popover .preset-chips .cn-button.active {
    border-color: var(--accent);
    background: var(--accent);
    color: var(--accent-contrast);
}
.report-popover .date-readout {
    margin-top: 11px;
    color: var(--text-subtle);
    font-family: var(--font-mono);
    font-size: 11px;
}
.report-popover .custom-dates {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
    margin-top: 12px;
}
.report-popover .custom-dates label {
    display: grid;
    gap: 5px;
    color: var(--text-subtle);
    font-size: 11px;
}
.report-popover .custom-dates .cn-date-picker-trigger {
    width: 100%;
    min-width: 0;
    justify-content: flex-start;
    padding-inline: 8px;
    font-size: 10px;
}
.report-popover .project-list {
    max-height: 260px;
    overflow-y: auto;
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    background: var(--surface-2);
    scrollbar-color: color-mix(in srgb, var(--text-subtle) 62%, transparent) transparent;
    scrollbar-gutter: stable;
    scrollbar-width: thin;
}
.report-popover .project-list::-webkit-scrollbar {
    width: 10px;
}
.report-popover .project-list::-webkit-scrollbar-track {
    border-radius: 0 var(--r-sm) var(--r-sm) 0;
    background: color-mix(in srgb, var(--surface-3) 72%, transparent);
}
.report-popover .project-list::-webkit-scrollbar-thumb {
    min-height: 44px;
    border: 3px solid transparent;
    border-radius: 999px;
    background: color-mix(in srgb, var(--text-subtle) 64%, transparent);
    background-clip: padding-box;
}
.report-popover .project-list::-webkit-scrollbar-thumb:hover {
    background: var(--scroll-thumb-hover);
    background-clip: padding-box;
}
.report-popover .project-row {
    display: flex;
    min-width: 0;
    align-items: flex-start;
    gap: 9px;
    padding: 9px;
    cursor: pointer;
}
.report-popover .project-row + .project-row {
    border-top: 1px solid var(--border);
}
.report-popover .project-row.selected {
    background: var(--accent-weak);
}
.report-popover .project-check {
    display: grid;
    width: 17px;
    height: 17px;
    flex: none;
    place-items: center;
    margin-top: 1px;
    padding: 0;
    border: 1.5px solid var(--border);
    border-radius: 4px;
    background: transparent;
    color: transparent;
    pointer-events: none;
}
.report-popover .project-row.selected .project-check {
    border-color: var(--accent);
    background: var(--accent);
    color: var(--accent-contrast);
}
.report-popover .project-meta {
    min-width: 0;
    flex: 1;
}
.report-popover .project-meta > div {
    display: flex;
    align-items: center;
    gap: 7px;
    min-width: 0;
}
.report-popover .project-meta strong {
    overflow: hidden;
    color: var(--text);
    font-size: 12.5px;
    text-overflow: ellipsis;
    white-space: nowrap;
}
.report-popover .project-meta span {
    flex: none;
    padding: 1px 6px;
    border: 1px solid var(--border);
    border-radius: 999px;
    color: var(--text-subtle);
    font-family: var(--font-mono);
    font-size: 9px;
}
.report-popover .project-meta small {
    display: block;
    overflow: hidden;
    margin-top: 3px;
    color: var(--text-subtle);
    font-family: var(--font-mono);
    font-size: 10px;
    text-overflow: ellipsis;
    white-space: nowrap;
}
.report-popover .project-remove {
    width: 22px;
    height: 22px;
    padding: 0;
    color: var(--text-subtle);
    font-size: 17px;
    line-height: 1;
    opacity: 0;
}
.report-popover .project-row:hover .project-remove {
    opacity: 1;
}
.report-popover .project-remove:hover {
    color: var(--u-crit);
}
.report-popover .popover-empty {
    margin: 20px 0;
    color: var(--text-subtle);
    font-size: 12px;
    text-align: center;
}
.report-popover .project-add {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto auto;
    gap: 7px;
    margin-top: 9px;
}
.report-popover .project-add .cn-input,
.report-popover .project-add .cn-button {
    height: 34px;
    font-size: 11px;
}
.report-popover .author-field {
    display: flex;
    min-height: 42px;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    padding: 7px;
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    background: var(--surface-2);
}
.report-popover .author-chip {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 3px 5px 3px 9px;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--surface-2);
    color: var(--text-muted);
    font-family: var(--font-mono);
    font-size: 11px;
}
.report-popover .author-chip button {
    padding: 0 2px;
    border: 0;
    background: transparent;
    color: var(--text-subtle);
    font-size: 14px;
    cursor: pointer;
}
.report-popover .author-field > input {
    min-width: 90px;
    flex: 1;
    padding: 4px;
    border: 0;
    outline: 0;
    background: transparent;
    color: var(--text);
    font-family: var(--font-mono);
    font-size: 11px;
}
</style>
