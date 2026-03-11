import org.gradle.api.Project
import org.gradle.kotlin.dsl.extra
import java.io.File
import java.io.FileInputStream
import java.util.Properties
import kotlin.io.path.absolutePathString
import kotlin.io.path.writeText

object BuildUtil {

    fun Project.isCentralSnapshotsPublish(): Boolean =
        gradle.startParameter.taskNames.any { taskName ->
            taskName.contains("centralSnapshots", ignoreCase = true)
        }

    fun Project.loadProperties(): Properties {
        val prop = loadPropertyFile()
        if (System.getenv("ENCODED_ANDROID_KEYSTORE")?.isNotEmpty() == true) {
            prepareSignKey(prop)
        }
        if (System.getenv("GPG_SECRET")?.isNotEmpty() == true) {
            prepareGpgKey(prop)
        }
        prop.forEach {
            extra.set(it.key as String, it.value as String)
        }
        return prop
    }

    private fun Project.loadPropertyFile(): Properties {
        val propFile = File(rootProject.rootDir, "local.properties")
        val prop = Properties()
        if (propFile.exists()) {
            prop.apply {
                FileInputStream(propFile).use {
                    load(it)
                }
            }
        }
        return prop
    }

    private fun Project.prepareSignKey(prop: Properties) {
        val keyStorePath = "/usr/local/keystore.jks"
        val file = File(keyStorePath)
        if (!file.exists()) {
            decodeBase64(System.getenv("ENCODED_ANDROID_KEYSTORE"), keyStorePath)
        }
        prop["SIGN_KEY_STORE_PATH"] = keyStorePath
        prop["SIGN_STORE_PASSWORD"] = System.getenv("SIGN_STORE_PASSWORD")
        prop["SIGN_KEY_ALIAS"] = System.getenv("SIGN_KEY_ALIAS")
        prop["SIGN_KEY_PASSWORD"] = System.getenv("SIGN_KEY_PASSWORD")
    }

    private fun Project.prepareGpgKey(prop: Properties) {
        val gpgPath = "/usr/local/secret.gpg"
        val gpgFile = File(gpgPath)
        if (!gpgFile.exists()) {
            val tmpPath = decodeBase64(System.getenv("GPG_SECRET"))
            providers.exec {
                commandLine = listOf(
                    "gpg",
                    "-o",
                    gpgPath,
                    "--dearmor",
                    tmpPath,
                )
            }
        }
        prop["sonatypeUsername"] = System.getenv("SONATYPEUSERNAME")
        prop["sonatypePassword"] = System.getenv("SONATYPEPASSWORD")
        prop["signing.secretKeyRingFile"] = "/usr/local/secret.gpg"
        prop["signing.keyId"] = System.getenv("GPG_KEY_ID")
        prop["signing.password"] = System.getenv("GPG_PWD")
    }

    private fun Project.decodeBase64(content: String, targetPath: String? = null): String {
        val sourceFile = kotlin.io.path.createTempFile()
        sourceFile.writeText(content)
        val outFile = if (targetPath == null) {
            File(kotlin.io.path.createTempFile().absolutePathString())
        } else {
            File(targetPath)
        }
        providers.exec {
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
