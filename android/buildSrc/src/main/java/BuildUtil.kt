import org.gradle.api.Project
import org.gradle.kotlin.dsl.extra
import java.io.File
import java.io.FileInputStream
import java.util.Properties
import kotlin.io.path.absolutePathString
import kotlin.io.path.writeText

object BuildUtil {

    fun Project.loadProperties() {
        loadPropertyFile()
        if (System.getenv("ENCODED_ANDROID_KEYSTORE")?.isNotEmpty() == true) {
            prepareSignKey()
        }
        if (System.getenv("GPG_SECRET")?.isNotEmpty() == true) {
            prepareGpgKey()
        }
    }

    private fun Project.loadPropertyFile() {
        val propFile = File(rootProject.rootDir, "local.properties")
        if (propFile.exists()) {
            val prop = Properties().apply {
                FileInputStream(propFile).use {
                    load(it)
                }
            }
            prop.forEach {
                extra.set(it.key as String, it.value as String)
            }
        }
    }

    private fun Project.prepareSignKey() {
        val keyStorePath = "/usr/local/keystore.jks"
        val file = File(keyStorePath)
        if (!file.exists()) {
            decodeBase64(System.getenv("ENCODED_ANDROID_KEYSTORE"), keyStorePath)
        }
        extra.set("SIGN_KEY_STORE_PATH", keyStorePath)
        extra.set("SIGN_STORE_PASSWORD", System.getenv("SIGN_STORE_PASSWORD"))
        extra.set("SIGN_KEY_ALIAS", System.getenv("SIGN_KEY_ALIAS"))
        extra.set("SIGN_KEY_PASSWORD", System.getenv("SIGN_KEY_PASSWORD"))
    }

    private fun Project.prepareGpgKey() {
        val gpgPath = "/usr/local/secret.gpg"
        val gpgFile = File(gpgPath)
        if (!gpgFile.exists()) {
            val tmpPath = decodeBase64(System.getenv("GPG_SECRET"))
            project.exec {
                commandLine = listOf(
                    "gpg",
                    "-o",
                    gpgPath,
                    "--dearmor",
                    tmpPath,
                )
            }
        }
        extra.set("sonatypeUsername", System.getenv("SONATYPEUSERNAME"))
        extra.set("sonatypePassword", System.getenv("SONATYPEPASSWORD"))
        extra.set("signing.secretKeyRingFile", "/usr/local/secret.gpg")
        extra.set("signing.keyId", System.getenv("GPG_KEY_ID"))
        extra.set("signing.password", System.getenv("GPG_PWD"))
    }

    private fun Project.decodeBase64(content: String, targetPath: String? = null): String {
        val sourceFile = kotlin.io.path.createTempFile()
        sourceFile.writeText(content)
        val outFile = if (targetPath == null) {
            File(kotlin.io.path.createTempFile().absolutePathString())
        } else {
            File(targetPath)
        }
        exec {
            commandLine = listOf(
                "base64",
                "-d",
                sourceFile.absolutePathString()
            )
            standardOutput = outFile.outputStream()
        }
        return outFile.absolutePath
    }
}