package com.modrinth.sandboxagent.transformer;

import org.objectweb.asm.ClassReader;
import org.objectweb.asm.ClassWriter;
import org.objectweb.asm.tree.ClassNode;
import org.objectweb.asm.tree.MethodNode;

import java.lang.instrument.ClassFileTransformer;
import java.security.ProtectionDomain;
import java.util.HashMap;

public class TransformerMap implements ClassFileTransformer {

    private final HashMap<String, HashMap<String, HashMap<String, Transformer>>> transformers = new HashMap<>();

    public void register(String className, String method, String desc, Transformer transformer) {
        HashMap<String, HashMap<String, Transformer>> byMethod = transformers.computeIfAbsent(className, k -> new HashMap<>());
        HashMap<String, Transformer> byDesc = byMethod.computeIfAbsent(method, k -> new HashMap<>());
        byDesc.put(desc, transformer);
    }

    @Override
    public byte[] transform(ClassLoader classLoader, String className, Class<?> clazz, ProtectionDomain protectionDomain, byte[] classBytes) {
        HashMap<String, HashMap<String, Transformer>> byMethod = transformers.get(className);
        if (byMethod == null) {
            return classBytes;
        }

        final ClassNode classNode = new ClassNode();
        ClassReader reader = new ClassReader(classBytes);
        reader.accept(classNode, 0);

        boolean modified = false;
        for (MethodNode method : classNode.methods) {
            HashMap<String, Transformer> byDesc = byMethod.get(method.name);
            if (byDesc == null) {
                continue;
            }

            Transformer transformer = byDesc.get(method.desc);
            if (transformer == null) {
                continue;
            }

            modified |= transformer.transformMethod(method.instructions.iterator());
        }

        if (!modified) {
            return classBytes;
        }

        ClassWriter classWriter = new ClassWriter(reader, ClassWriter.COMPUTE_FRAMES | ClassWriter.COMPUTE_MAXS);
        classNode.accept(classWriter);
        return classWriter.toByteArray();



    }
}
