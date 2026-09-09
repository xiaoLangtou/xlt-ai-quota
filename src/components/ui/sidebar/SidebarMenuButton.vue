<script setup lang="ts">
import { Button } from "@/components/ui/button";
defineProps<{ active?: boolean }>();
</script>
<template><Button :variant="active ? 'secondary' : 'ghost'" class="cn-sidebar-menu-button"><slot /></Button></template>
