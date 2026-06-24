FROM node:20-slim AS frontend
WORKDIR /client
COPY client/package*.json ./
RUN npm install
COPY client/ ./
RUN npm run build

FROM node:20-slim
WORKDIR /app
COPY package*.json ./
RUN npm install --production
COPY server.js ./
COPY --from=frontend /client/dist ./public
EXPOSE 3000
CMD ["node", "server.js"]
