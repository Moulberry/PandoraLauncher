package com.modrinth.sandboxagent;

import java.lang.instrument.ClassFileTransformer;
import java.lang.instrument.IllegalClassFormatException;
import java.lang.instrument.Instrumentation;
import java.net.URI;
import java.security.ProtectionDomain;
import java.util.List;
import java.util.ListIterator;
import java.util.concurrent.CompletableFuture;

import com.modrinth.sandboxagent.transformer.TransformerMap;
import org.objectweb.asm.*;
import org.objectweb.asm.tree.*;

public class SandboxAgent {
    private static SandboxApi sandboxApi;

	public static void premain(String agentArgs, Instrumentation inst) {
        String[] split = agentArgs.split(",");
        if (split.length != 2) {
            throw new RuntimeException("Expected 2 arguments");
        }
        String secret = split[0];
        int port = Integer.parseInt(split[1]);
        sandboxApi = new SandboxApi(secret, port);

		instrument(inst);
	}

	private static void instrument(Instrumentation inst) {
        TransformerMap transformerMap = new TransformerMap();

        // openURI
        transformerMap.register("net/minecraft/util/Util$OS", "openUri", "(Ljava/net/URI;)V", SandboxAgent::transformOpenUri); // Mojang 1.21.11
        transformerMap.register("net/minecraft/Util$OS", "openUri", "(Ljava/net/URI;)V", SandboxAgent::transformOpenUri); // Mojang
        transformerMap.register("net/minecraft/class_156$class_158", "method_670", "(Ljava/net/URI;)V", SandboxAgent::transformOpenUri); // Intermediary
        transformerMap.register("net/minecraft/src/C_5322_/C_5330_", "m_137646_", "(Ljava/net/URI;)V", SandboxAgent::transformOpenUri);

		inst.addTransformer(transformerMap);
        inst.addTransformer(new ClassFileTransformer() {
            @Override
            public byte[] transform(ClassLoader loader, String className, Class<?> classBeingRedefined, ProtectionDomain protectionDomain, byte[] classfileBuffer) throws IllegalClassFormatException {
                System.out.println("Got class: " + className);
                return null;
            }
        });
	}

    private static boolean transformOpenUri(ListIterator<AbstractInsnNode> it) {
        final LabelNode after = new LabelNode();
        it.add(new VarInsnNode(Opcodes.ALOAD, 1));
        it.add(new MethodInsnNode(Opcodes.INVOKESTATIC, "com/modrinth/sandboxagent/SandboxAgent", "openUri", "(Ljava/net/URI;)Z"));
        it.add(new JumpInsnNode(Opcodes.IFEQ, after)); // If false, jump to after to skip early return
        it.add(new InsnNode(Opcodes.RETURN));
        it.add(after);
        return true;
    }

    public static boolean openUri(URI uri) {
        String scheme = uri.getScheme();
        if (scheme.equals("http") || scheme.equals("https") || scheme.equals("file")) {
            CompletableFuture.runAsync(() -> sandboxApi.openUri(uri));
            return true;
        }
        return false;
    }

}
