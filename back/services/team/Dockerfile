FROM gradle:8.5-jdk17 AS build

WORKDIR /app
COPY . .

RUN gradle build --no-daemon

FROM eclipse-temurin:17-jre

WORKDIR /app

COPY --from=build /app/build/libs/*.jar app.jar

EXPOSE 15003

CMD ["java", "-jar", "app.jar"]