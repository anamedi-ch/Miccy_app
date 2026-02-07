import React from "react";

interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: "primary" | "secondary" | "danger" | "ghost";
  size?: "sm" | "md" | "lg";
}

export const Button: React.FC<ButtonProps> = ({
  children,
  className = "",
  variant = "primary",
  size = "md",
  ...props
}) => {
  const baseClasses =
    "font-medium rounded-lg border focus:outline-none transition-all duration-200 disabled:opacity-50 disabled:cursor-not-allowed cursor-pointer";

  const variantClasses = {
    primary:
      "text-white bg-background-ui border-background-ui shadow-sm hover:bg-background-ui/90 hover:shadow-md active:scale-[0.98] focus:ring-2 focus:ring-background-ui/40 focus:ring-offset-2 focus:ring-offset-background",
    secondary:
      "bg-background border-mid-gray/25 text-text hover:bg-mid-gray/10 hover:border-mid-gray/40 active:scale-[0.98] focus:ring-2 focus:ring-logo-primary/20 focus:ring-offset-2 focus:ring-offset-background focus:border-logo-primary",
    danger:
      "text-white bg-red-600 border-red-600 shadow-sm hover:bg-red-700 hover:border-red-700 hover:shadow-md active:scale-[0.98] focus:ring-2 focus:ring-red-500/40 focus:ring-offset-2 focus:ring-offset-background",
    ghost:
      "text-current border-transparent hover:bg-mid-gray/10 active:scale-[0.98] focus:ring-2 focus:ring-logo-primary/20 focus:ring-offset-2 focus:ring-offset-background",
  };

  const sizeClasses = {
    sm: "px-3 py-1.5 text-xs",
    md: "px-4 py-2 text-sm",
    lg: "px-5 py-2.5 text-base",
  };

  return (
    <button
      className={`${baseClasses} ${variantClasses[variant]} ${sizeClasses[size]} ${className}`}
      {...props}
    >
      {children}
    </button>
  );
};
