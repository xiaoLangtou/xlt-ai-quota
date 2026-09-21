import { defineStore } from "pinia";
import { ref } from "vue";
import type { SkillProject, SkillRoot, SkillSummary } from "@/types/skills";
import { skillsApi, skillsErrorMessage } from "@/api/skills";

export const useSkillsStore = defineStore("skills", () => {
  const roots = ref<SkillRoot[]>([]);
  const projects = ref<SkillProject[]>([]);
  const skills = ref<SkillSummary[]>([]);
  const loading = ref(false);
  const error = ref("");

  async function refresh() {
    loading.value = true;
    error.value = "";
    try {
      const [rootList, projectList, skillList] = await Promise.all([
        skillsApi.listRoots(),
        skillsApi.listProjects(),
        skillsApi.listSkills(),
      ]);
      roots.value = rootList;
      projects.value = projectList;
      skills.value = skillList;
    } catch (caught) {
      error.value = skillsErrorMessage(caught);
    } finally {
      loading.value = false;
    }
  }

  return { roots, projects, skills, loading, error, refresh };
});
