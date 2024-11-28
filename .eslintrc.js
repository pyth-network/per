module.exports = {
  root: true,
  parser: "@typescript-eslint/parser",
  plugins: ["@typescript-eslint"],
  parserOptions: {
    project: "./tsconfig.json",
  },
  rules: {
    "@typescript-eslint/no-misused-promises": "error",
  },
  extends: ["eslint:recommended", "plugin:@typescript-eslint/recommended"],
};
