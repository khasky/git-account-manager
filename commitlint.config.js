/** @type {import("@commitlint/types").UserConfig} */
export default {
  extends: ["@commitlint/config-conventional"],
  rules: {
    // URLs and detailed explanations in the body/footer should not fail on
    // line length — only the header (type/scope/subject) shape is enforced.
    "body-max-line-length": [0, "always"],
    "footer-max-line-length": [0, "always"],
  },
};
