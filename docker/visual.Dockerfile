# Визуальные тесты Storybook: тот же образ, что в CI (Playwright + Linux), чтобы эталоны совпадали.
# Версия образа должна совпадать с @playwright/test в web/package.json; Node в образе — как в .node-version.
FROM mcr.microsoft.com/playwright:v1.64.1-noble

WORKDIR /app/web
COPY web/package.json web/package-lock.json ./
RUN npm ci

COPY design /app/design
COPY web ./
RUN npm run build-storybook

CMD ["npx", "playwright", "test"]
