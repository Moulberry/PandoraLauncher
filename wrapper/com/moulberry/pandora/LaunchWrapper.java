package com.moulberry.pandora;

import java.util.Scanner;
import java.util.List;
import java.util.ArrayList;

public class LaunchWrapper {
    public static void main(String[] args) throws Throwable {
        Scanner scanner = new Scanner(System.in);
        List<String> arguments = new ArrayList<>();
        while (true) {
            String command = scanner.nextLine();

            if (command.equals("exit")) {
                return;
            }

            String value = scanner.nextLine();

            if (command.equals("arg")) {
                arguments.add(value);
            } else if (command.equals("property")) {
                String propertyValue = scanner.nextLine();
                System.setProperty(value, propertyValue);
            } else if (command.equals("printproperty")) {
                System.out.println(System.getProperty(value));
            } else if (command.equals("launch")) {
                String[] argumentsArray = arguments.toArray(new String[0]);
                Class.forName(value).getDeclaredMethod("main", String[].class).invoke(null, (Object) argumentsArray);
                return;
            }
        }
    }
}
