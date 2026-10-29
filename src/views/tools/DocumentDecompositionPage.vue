<script setup lang="ts">
import { useRouter } from "vue-router";
import DocumentDecompositionView from "@/views/DocumentDecompositionView.vue";

const router = useRouter();
</script>

<template>
  <UDashboardPanel id="document-decomposition" :ui="{ body: 'p-0 sm:p-0 gap-0 overflow-hidden' }">
    <template #header>
      <UDashboardNavbar title="文档拆解" description="从 PRD、会议纪要或需求说明生成可确认的待办">
        <template #leading>
          <UDashboardSidebarCollapse />
        </template>
        <template #right>
          <UButton
            icon="i-lucide-arrow-left"
            color="neutral"
            variant="outline"
            size="sm"
            class="whitespace-nowrap"
            @click="router.push('/calendar')"
          >
            返回日历
          </UButton>
        </template>
      </UDashboardNavbar>
    </template>

    <template #body>
      <DocumentDecompositionView />
    </template>
  </UDashboardPanel>
</template>
