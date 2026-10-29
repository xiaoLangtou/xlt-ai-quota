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
  let refreshRequest = 0;

  async function refresh() {
    const request = ++refreshRequest;
    loading.value = true;
    error.value = "";
    try {
      const [rootList, projectList, skillList] = await Promise.all([
        skillsApi.listRoots(),
        skillsApi.listProjects(),
        skillsApi.listSkills(),
      ]);
      if (request !== refreshRequest) return;
      roots.value = rootList;
      projects.value = projectList;
      skills.value = skillList;
    } catch (caught) {
      if (request !== refreshRequest) return;
      error.value = skillsErrorMessage(caught);
    } finally {
      if (request === refreshRequest) loading.value = false;
    }
  }

  return { roots, projects, skills, loading, error, refresh };
});
