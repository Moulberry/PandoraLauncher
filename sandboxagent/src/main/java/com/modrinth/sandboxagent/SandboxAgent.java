package com.modrinth.sandboxagent;

import java.lang.instrument.Instrumentation;
import java.net.URI;
import java.util.ListIterator;
import java.util.concurrent.CompletableFuture;

import com.modrinth.sandboxagent.transformer.TransformerMap;
import org.objectweb.asm.*;
import org.objectweb.asm.tree.AbstractInsnNode;
import org.objectweb.asm.tree.InsnNode;
import org.objectweb.asm.tree.JumpInsnNode;
import org.objectweb.asm.tree.LabelNode;
import org.objectweb.asm.tree.MethodInsnNode;
import org.objectweb.asm.tree.VarInsnNode;

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

        transformerMap.register("net/minecraft/util/Util$OS", "openUri", "(Ljava/net/URI;)V", SandboxAgent::transformOpenUri);
        // todo: net/minecraft/util/Util$OS obfuscated
        // todo: net/minecraft/Util$OS obfuscated

		inst.addTransformer(transformerMap);
	}

    private static boolean transformOpenUri(ListIterator<AbstractInsnNode> it) {
        final LabelNode after = new LabelNode();
        it.add(new VarInsnNode(Opcodes.ALOAD, 1));
        it.add(new MethodInsnNode(Opcodes.INVOKESTATIC, "com/modrinth/sandboxagent/SandboxAgent", "openUri", "(Ljava/net/URI;)Z"));
        it.add(new JumpInsnNode(Opcodes.IFNE, after));
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
