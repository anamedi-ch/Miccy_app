import React from "react";

interface InputProps extends React.InputHTMLAttributes<HTMLInputElement> {
  variant?: "default" | "compact";
}

export const Input: React.FC<InputProps> = ({
  className = "",
  variant = "default",
  disabled,
  ...props
}) => {
  const baseClasses =
    "text-sm font-medium bg-background border rounded-lg text-start transition-all duration-200";

  const interactiveClasses = disabled
    ? "opacity-60 cursor-not-allowed border-mid-gray/30"
    : "border-mid-gray/30 hover:border-mid-gray/50 focus:outline-none focus:ring-2 focus:ring-logo-primary/20 focus:border-logo-primary";

  const variantClasses = {
    default: "px-3 py-2 min-h-[40px]",
    compact: "px-2.5 py-1.5 min-h-[36px]",
  } as const;

  return (
    <input
      className={`${baseClasses} ${variantClasses[variant]} ${interactiveClasses} ${className}`}
      disabled={disabled}
      {...props}
    />
  );
};
