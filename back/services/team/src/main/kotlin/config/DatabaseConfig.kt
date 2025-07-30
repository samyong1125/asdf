package com.asdf.config

import com.mongodb.kotlin.client.coroutine.MongoClient
import com.mongodb.kotlin.client.coroutine.MongoDatabase
import io.ktor.server.application.*

object DatabaseConfig {
    private lateinit var client: MongoClient
    private lateinit var database: MongoDatabase
    
    fun init(application: Application) {
        val host = application.environment.config.property("mongodb.host").getString()
        val port = application.environment.config.property("mongodb.port").getString()
        val databaseName = application.environment.config.property("mongodb.database").getString()
        
        val connectionString = "mongodb://$host:$port"
        client = MongoClient.create(connectionString)
        database = client.getDatabase(databaseName)
        
        application.log.info("MongoDB 연결 설정 완료: $connectionString/$databaseName")
    }
    
    fun getDatabase(): MongoDatabase = database
    
    suspend fun testConnection(): Boolean {
        return try {
            database.runCommand(org.bson.Document("ping", 1))
            true
        } catch (e: Exception) {
            false
        }
    }
    
    fun close() {
        if (::client.isInitialized) {
            client.close()
        }
    }
}