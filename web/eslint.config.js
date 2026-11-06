import ts from 'typescript-eslint';
import hooks from 'eslint-plugin-react-hooks';
import a11y from 'eslint-plugin-jsx-a11y';
export default ts.config(
  { ignores: ['dist/**', 'coverage/**', 'node_modules/**'] },
  ...ts.configs.recommended,
  {
    files: ['**/*.{ts,tsx}'],
    plugins: {
      'react-hooks': hooks,
      'jsx-a11y': a11y,
      copy: {
        rules: {
          translated: {
            create(context) {
              return {
                JSXText(node) {
                  if (node.value.trim())
                    context.report({
                      node,
                      message: 'Use a locale key for UI copy.',
                    });
                },
                JSXAttribute(node) {
                  if (
                    ['title', 'placeholder', 'aria-label', 'alt'].includes(
                      node.name.name,
                    ) &&
                    node.value?.type === 'Literal'
                  )
                    context.report({
                      node,
                      message: 'Translate this attribute.',
                    });
                },
              };
            },
          },
        },
      },
    },
    rules: {
      ...hooks.configs.recommended.rules,
      ...a11y.configs.recommended.rules,
      'copy/translated': 'error',
    },
  },
);
