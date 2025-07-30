package com.asdf

import com.asdf.config.DatabaseConfig
import io.ktor.serialization.kotlinx.json.*
import io.ktor.server.application.*
import io.ktor.server.plugins.contentnegotiation.*
import kotlinx.serialization.json.Json
import kotlinx.serialization.modules.SerializersModule
import kotlinx.serialization.modules.contextual
import org.bson.types.ObjectId
import kotlinx.datetime.Instant

fun main(args: Array<String>) {
    io.ktor.server.netty.EngineMain.main(args)
}

fun Application.module() {
    configureSerialization()
    configureDatabase()
    configureRouting()
}

fun Application.configureSerialization() {
    install(ContentNegotiation) {
        json(Json {
            serializersModule = SerializersModule {
                contextual(ObjectId::class) { ObjectIdSerializer }
                contextual(Instant::class) { InstantSerializer }
            }
            prettyPrint = true
            isLenient = true
        })
    }
}

fun Application.configureDatabase() {
    DatabaseConfig.init(this)
    
    monitor.subscribe(ApplicationStopping) {
        DatabaseConfig.close()
    }
}
