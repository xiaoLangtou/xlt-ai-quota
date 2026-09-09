import type { VariantProps } from "class-variance-authority";
import { cva } from "class-variance-authority";

export { default as Button } from "./Button.vue";

export const buttonVariants = cva("cn-button", {
  variants: {
    variant: { default: "cn-button-default", outline: "cn-button-outline", secondary: "cn-button-secondary", ghost: "cn-button-ghost", destructive: "cn-button-destructive" },
    size: { default: "cn-button-default-size", sm: "cn-button-sm", lg: "cn-button-lg", icon: "cn-button-icon" },
  },
  defaultVariants: { variant: "default", size: "default" },
});
export type ButtonVariants = VariantProps<typeof buttonVariants>;
